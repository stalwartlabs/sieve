/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::char::REPLACEMENT_CHARACTER;

use crate::MAX_MATCH_VARIABLES;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobPattern {
    pattern: Vec<PatternChar>,
    to_lower: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatternChar {
    WildcardMany { num: usize, match_pos: usize },
    WildcardSingle { match_pos: usize },
    Char { char: char, match_pos: usize },
}

impl GlobPattern {
    pub fn compile(pattern: &str, to_lower: bool) -> Self {
        let mut chars = Vec::new();
        let mut is_escaped = false;
        let mut str = pattern.chars().peekable();

        while let Some(char) = str.next() {
            match char {
                '*' if !is_escaped => {
                    let mut num = 1;
                    while let Some('*') = str.peek() {
                        num += 1;
                        str.next();
                    }
                    chars.push(PatternChar::WildcardMany { num, match_pos: 0 });
                }
                '?' if !is_escaped => {
                    chars.push(PatternChar::WildcardSingle { match_pos: 0 });
                }
                '\\' if !is_escaped => {
                    is_escaped = true;
                    continue;
                }
                _ => {
                    if is_escaped {
                        is_escaped = false;
                    }
                    if to_lower && char.is_uppercase() {
                        for char in char.to_lowercase() {
                            chars.push(PatternChar::Char { char, match_pos: 0 });
                        }
                    } else {
                        chars.push(PatternChar::Char { char, match_pos: 0 });
                    }
                }
            }
        }

        GlobPattern {
            pattern: chars,
            to_lower,
        }
    }

    // Credits: Algorithm ported from https://research.swtch.com/glob
    pub fn matches(&self, value: &str) -> bool {
        let value = if self.to_lower {
            value.to_lowercase().chars().collect::<Vec<_>>()
        } else {
            value.chars().collect::<Vec<_>>()
        };

        let mut px = 0;
        let mut nx = 0;
        let mut next_px = 0;
        let mut next_nx = 0;

        while px < self.pattern.len() || nx < value.len() {
            match self.pattern.get(px) {
                Some(PatternChar::Char { char, .. }) => {
                    if matches!(value.get(nx), Some(nc) if nc == char ) {
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardSingle { .. }) => {
                    if nx < value.len() {
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardMany { .. }) => {
                    next_px = px;
                    next_nx = nx + 1;
                    px += 1;
                    continue;
                }
                _ => (),
            }
            if 0 < next_nx && next_nx <= value.len() {
                px = next_px;
                nx = next_nx;
                continue;
            }
            return false;
        }
        true
    }

    pub fn capture(
        mut self,
        value_: &str,
        capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        let value = if self.to_lower {
            let mut value = Vec::with_capacity(value_.len());
            for char in value_.chars() {
                if char.is_uppercase() {
                    for (pos, lowerchar) in char.to_lowercase().enumerate() {
                        value.push((
                            lowerchar,
                            if pos == 0 {
                                char
                            } else {
                                REPLACEMENT_CHARACTER
                            },
                        ));
                    }
                } else {
                    value.push((char, char));
                }
            }
            value
        } else {
            value_.chars().map(|char| (char, char)).collect::<Vec<_>>()
        };

        let mut px = 0;
        let mut nx = 0;
        let mut next_px = 0;
        let mut next_nx = 0;

        while px < self.pattern.len() || nx < value.len() {
            match self.pattern.get_mut(px) {
                Some(PatternChar::Char { char, match_pos }) => {
                    if matches!(value.get(nx), Some(nc) if &nc.0 == char ) {
                        *match_pos = nx;
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardSingle { match_pos }) => {
                    if nx < value.len() {
                        *match_pos = nx;
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardMany { match_pos, .. }) => {
                    *match_pos = nx;
                    next_px = px;
                    next_nx = nx + 1;
                    px += 1;
                    continue;
                }
                _ => (),
            }
            if 0 < next_nx && next_nx <= value.len() {
                px = next_px;
                nx = next_nx;
                continue;
            }
            return false;
        }

        let mut last_pos = 0;

        captured_values.clear();
        if capture_positions & 1 != 0 {
            captured_values.push((0usize, value_.to_string()));
        }

        let mut wildcard_pos: usize = 1;
        for item in &self.pattern {
            if wildcard_pos <= MAX_MATCH_VARIABLES as usize {
                last_pos = match item {
                    PatternChar::WildcardMany { mut num, match_pos } => {
                        while num > 1 {
                            if capture_positions & (1 << wildcard_pos) != 0 {
                                captured_values.push((wildcard_pos, String::with_capacity(0)));
                            }
                            wildcard_pos += 1;
                            num -= 1;
                        }

                        if capture_positions & (1 << wildcard_pos) != 0 {
                            if let Some(range) = value.get(last_pos..*match_pos) {
                                captured_values.push((
                                    wildcard_pos,
                                    range
                                        .iter()
                                        .filter_map(|(_, char)| {
                                            if char != &REPLACEMENT_CHARACTER {
                                                Some(char)
                                            } else {
                                                None
                                            }
                                        })
                                        .collect::<String>(),
                                ));
                            } else {
                                debug_assert!(false, "Glob pattern failure.");
                                return false;
                            }
                        }
                        wildcard_pos += 1;
                        match_pos
                    }
                    PatternChar::WildcardSingle { match_pos } => {
                        if capture_positions & (1 << wildcard_pos) != 0 {
                            if let Some((char, orig_char)) = value.get(*match_pos) {
                                captured_values.push((
                                    wildcard_pos,
                                    (if orig_char != &REPLACEMENT_CHARACTER {
                                        orig_char
                                    } else {
                                        char
                                    })
                                    .to_string(),
                                ));
                            } else {
                                debug_assert!(false, "Glob pattern failure.");
                                return false;
                            }
                        }
                        wildcard_pos += 1;
                        match_pos
                    }
                    PatternChar::Char { match_pos, .. } => match_pos,
                } + 1;
            } else {
                break;
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::tests::glob::GlobPattern;

    #[test]
    fn glob_match() {
        for (value, pattern, expected_result) in [
            (
                "frop.......frop.........frop....",
                "?*frop*",
                vec!["f", "rop.......", ".........frop...."],
            ),
            ("frop:frup:frop", "*:*:*", vec!["frop", "frup", "frop"]),
            (
                "a b c d e f g",
                "? ? ? ? ? ? ?",
                vec!["a", "b", "c", "d", "e", "f", "g"],
            ),
            ("puk pok puk pok", "pu*ok", vec!["k pok puk p"]),
            ("snot kip snot", "snot*snot", vec![" kip "]),
            (
                "klopfropstroptop",
                "*fr??*top",
                vec!["klop", "o", "p", "strop"],
            ),
            ("toptoptop", "*top", vec!["toptop"]),
            (
                "Fehlende Straße zur Karte hinzufügen",
                "FEHLENDE * ZUR Karte HINZUFÜGEN",
                vec!["Straße"],
            ),
        ] {
            let p = GlobPattern::compile(pattern, true);
            let mut match_values = Vec::new();
            assert!(
                p.clone().capture(value, u64::MAX ^ 1, &mut match_values),
                "{value:?} {pattern:?}",
            );

            assert_eq!(
                match_values.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
                expected_result,
                "{value:?} {pattern:?}",
            );
            assert!(p.matches(value), "{value:?} {pattern:?}",);
        }
    }
}
