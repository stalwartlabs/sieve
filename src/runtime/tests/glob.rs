/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::comparator::casemap_eq;
use crate::MAX_MATCH_VARIABLES;
use std::char::REPLACEMENT_CHARACTER;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CompiledGlob {
    shape: Option<GlobShape>,
    pattern: GlobPattern,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum GlobShape {
    Literal(String),
    Prefix(String),
    Suffix(String),
    Contains(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct GlobPattern {
    pattern: Vec<PatternChar>,
    to_lower: bool,
    is_ascii: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PatternChar {
    WildcardMany(u32),
    WildcardSingle,
    Char(char),
}

impl CompiledGlob {
    pub fn compile(pattern: &str, to_lower: bool) -> Self {
        let pattern = GlobPattern::compile(pattern, to_lower);

        CompiledGlob {
            shape: pattern.classify(),
            pattern,
        }
    }

    pub fn matches(&self, value: &str) -> bool {
        match &self.shape {
            Some(shape) => shape.matches(value, self.pattern.to_lower),
            None => self.pattern.matches(value),
        }
    }

    pub fn capture(
        &self,
        value: &str,
        capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        self.pattern
            .capture(value, capture_positions, captured_values)
    }
}

impl GlobShape {
    fn matches(&self, value: &str, to_lower: bool) -> bool {
        match self {
            GlobShape::Literal(literal) => {
                if to_lower {
                    casemap_eq(value, literal)
                } else {
                    value == literal
                }
            }
            GlobShape::Prefix(prefix) => {
                if !to_lower {
                    value.starts_with(prefix.as_str())
                } else if value.is_ascii() && prefix.is_ascii() {
                    value.len() >= prefix.len()
                        && value.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
                } else {
                    value.to_lowercase().starts_with(prefix.as_str())
                }
            }
            GlobShape::Suffix(suffix) => {
                if !to_lower {
                    value.ends_with(suffix.as_str())
                } else if value.is_ascii() && suffix.is_ascii() {
                    value.len() >= suffix.len()
                        && value.as_bytes()[value.len() - suffix.len()..]
                            .eq_ignore_ascii_case(suffix.as_bytes())
                } else {
                    value.to_lowercase().ends_with(suffix.as_str())
                }
            }
            GlobShape::Contains(needle) => {
                if !to_lower {
                    value.contains(needle.as_str())
                } else {
                    value.to_lowercase().contains(needle.as_str())
                }
            }
        }
    }
}

impl GlobPattern {
    pub fn compile(pattern: &str, to_lower: bool) -> Self {
        let mut chars = Vec::with_capacity(pattern.len());
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
                    chars.push(PatternChar::WildcardMany(num));
                }
                '?' if !is_escaped => {
                    chars.push(PatternChar::WildcardSingle);
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
                            chars.push(PatternChar::Char(char));
                        }
                    } else {
                        chars.push(PatternChar::Char(char));
                    }
                }
            }
        }

        let is_ascii = chars.iter().all(|c| match c {
            PatternChar::Char(char) => char.is_ascii(),
            _ => true,
        });

        GlobPattern {
            pattern: chars,
            to_lower,
            is_ascii,
        }
    }

    fn classify(&self) -> Option<GlobShape> {
        let mut body = self.pattern.as_slice();
        let leading = matches!(body.first(), Some(PatternChar::WildcardMany(_)));
        if leading {
            body = &body[1..];
        }
        let trailing = matches!(body.last(), Some(PatternChar::WildcardMany(_)));
        if trailing {
            body = &body[..body.len() - 1];
        }

        let mut literal = String::with_capacity(body.len());
        for item in body {
            match item {
                PatternChar::Char(char) => literal.push(*char),
                _ => return None,
            }
        }

        Some(match (leading, trailing) {
            (false, false) => GlobShape::Literal(literal),
            (false, true) => GlobShape::Prefix(literal),
            (true, false) => GlobShape::Suffix(literal),
            (true, true) => GlobShape::Contains(literal),
        })
    }

    // Credits: Algorithm ported from https://research.swtch.com/glob
    pub fn matches(&self, value: &str) -> bool {
        if self.is_ascii && value.is_ascii() {
            self.matches_ascii(value.as_bytes())
        } else {
            self.matches_chars(&self.fold(value))
        }
    }

    fn fold(&self, value: &str) -> Vec<char> {
        if self.to_lower {
            value.to_lowercase().chars().collect::<Vec<_>>()
        } else {
            value.chars().collect::<Vec<_>>()
        }
    }

    fn matches_ascii(&self, value: &[u8]) -> bool {
        let mut px = 0;
        let mut nx = 0;
        let mut next_px = 0;
        let mut next_nx = 0;

        while px < self.pattern.len() || nx < value.len() {
            match self.pattern.get(px) {
                Some(PatternChar::Char(char)) => {
                    let char = *char as u8;
                    let matched = if self.to_lower {
                        matches!(value.get(nx), Some(nc) if nc.to_ascii_lowercase() == char)
                    } else {
                        matches!(value.get(nx), Some(nc) if *nc == char)
                    };
                    if matched {
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardSingle) if nx < value.len() => {
                    px += 1;
                    nx += 1;
                    continue;
                }
                Some(PatternChar::WildcardMany(_)) => {
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

    fn matches_chars(&self, value: &[char]) -> bool {
        let mut px = 0;
        let mut nx = 0;
        let mut next_px = 0;
        let mut next_nx = 0;

        while px < self.pattern.len() || nx < value.len() {
            match self.pattern.get(px) {
                Some(PatternChar::Char(char)) => {
                    if matches!(value.get(nx), Some(nc) if nc == char) {
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardSingle) if nx < value.len() => {
                    px += 1;
                    nx += 1;
                    continue;
                }
                Some(PatternChar::WildcardMany(_)) => {
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
        &self,
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

        let mut match_pos = vec![0usize; self.pattern.len()];

        let mut px = 0;
        let mut nx = 0;
        let mut next_px = 0;
        let mut next_nx = 0;

        while px < self.pattern.len() || nx < value.len() {
            match self.pattern.get(px) {
                Some(PatternChar::Char(char)) => {
                    if matches!(value.get(nx), Some(nc) if &nc.0 == char) {
                        match_pos[px] = nx;
                        px += 1;
                        nx += 1;
                        continue;
                    }
                }
                Some(PatternChar::WildcardSingle) if nx < value.len() => {
                    match_pos[px] = nx;
                    px += 1;
                    nx += 1;
                    continue;
                }
                Some(PatternChar::WildcardMany(_)) => {
                    match_pos[px] = nx;
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
        for (px, item) in self.pattern.iter().enumerate() {
            if wildcard_pos > MAX_MATCH_VARIABLES as usize {
                break;
            }
            let match_pos = match_pos[px];
            last_pos = match item {
                PatternChar::WildcardMany(num) => {
                    let mut num = *num;
                    while num > 1 {
                        if capture_positions & (1 << wildcard_pos) != 0 {
                            captured_values.push((wildcard_pos, String::with_capacity(0)));
                        }
                        wildcard_pos += 1;
                        num -= 1;
                    }

                    if capture_positions & (1 << wildcard_pos) != 0 {
                        if let Some(range) = value.get(last_pos..match_pos) {
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
                PatternChar::WildcardSingle => {
                    if capture_positions & (1 << wildcard_pos) != 0 {
                        if let Some((char, orig_char)) = value.get(match_pos) {
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
                PatternChar::Char(_) => match_pos,
            } + 1;
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use crate::runtime::tests::glob::{CompiledGlob, GlobPattern};

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
                p.capture(value, u64::MAX ^ 1, &mut match_values),
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

    #[test]
    fn glob_shapes() {
        for (pattern, value, expected) in [
            ("hello", "hello", true),
            ("hello", "HeLLo", true),
            ("hello", "hello!", false),
            ("hello*", "hello world", true),
            ("hello*", "HELLO world", true),
            ("hello*", "well hello", false),
            ("*world", "hello world", true),
            ("*world", "hello WORLD", true),
            ("*world", "world order", false),
            ("*ell*", "hello", true),
            ("*ell*", "HELLO", true),
            ("*ell*", "hallo", false),
            ("*", "anything", true),
            ("*", "", true),
            ("", "", true),
            ("", "x", false),
            ("**hello**", "say hello now", true),
            ("\\*literal\\*", "*literal*", true),
            ("\\*literal\\*", "xliteralx", false),
            ("*Straße*", "Die Straße hier", true),
            ("*STRASSE*", "Die Straße hier", false),
            ("*ü*", "Grün", true),
        ] {
            for to_lower in [true, false] {
                let compiled = CompiledGlob::compile(pattern, to_lower);
                let general = GlobPattern::compile(pattern, to_lower);
                assert_eq!(
                    compiled.matches(value),
                    general.matches(value),
                    "{pattern:?} {value:?} to_lower={to_lower} disagreed with the general path",
                );
                if to_lower {
                    assert_eq!(compiled.matches(value), expected, "{pattern:?} {value:?}");
                }

                let mut shape_captures = Vec::new();
                let mut general_captures = Vec::new();
                assert_eq!(
                    compiled.capture(value, u64::MAX, &mut shape_captures),
                    general.capture(value, u64::MAX, &mut general_captures),
                    "{pattern:?} {value:?} to_lower={to_lower} capture disagreed",
                );
                assert_eq!(
                    shape_captures, general_captures,
                    "{pattern:?} {value:?} to_lower={to_lower} captures differ",
                );
            }
        }
    }

    #[test]
    fn glob_shape_honours_case_sensitivity() {
        for (pattern, value, octet_expected) in [
            ("cafe*", "Cafe RESUME Hello", false),
            ("Cafe*", "Cafe RESUME Hello", true),
            ("*FOO*", "aFOObar", true),
            ("*foo*", "aFOObar", false),
            ("*hello there", "say Hello there", false),
            ("say hello there", "say Hello there", false),
        ] {
            assert_eq!(
                CompiledGlob::compile(pattern, false).matches(value),
                octet_expected,
                "octet {pattern:?} {value:?}",
            );
            assert!(
                CompiledGlob::compile(pattern, true).matches(value),
                "casemap {pattern:?} {value:?}",
            );
        }
    }

    #[test]
    fn glob_shape_star_runs_keep_capture_slots() {
        for (pattern, value, expected) in [
            ("**", "say Hello there", vec!["", "say Hello there"]),
            ("say**", "say Hello there", vec!["", " Hello there"]),
            ("**there", "say Hello there", vec!["", "say Hello "]),
            ("say***", "say Hello there", vec!["", "", " Hello there"]),
            ("***", "abc", vec!["", "", "abc"]),
        ] {
            let mut captures = Vec::new();
            assert!(
                CompiledGlob::compile(pattern, true).capture(value, u64::MAX ^ 1, &mut captures),
                "{pattern:?} {value:?}",
            );
            assert_eq!(
                captures.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
                expected,
                "{pattern:?} {value:?}",
            );
        }
    }

    #[test]
    fn glob_shape_captures_match_general_path() {
        for (pattern, value) in [
            ("cafe*", "Cafe RESUME Hello"),
            ("*FOO*", "aFOObar"),
            ("*foo*", "aFOObar"),
            ("say hello there", "say Hello there"),
            ("*hello there", "say Hello there"),
            ("**", "say Hello there"),
            ("say**", "say Hello there"),
            ("**there", "say Hello there"),
            ("say***", "say Hello there"),
            ("alpha**", "alpha*beta?gamma"),
            ("***", "abc"),
        ] {
            for to_lower in [true, false] {
                let compiled = CompiledGlob::compile(pattern, to_lower);
                let general = GlobPattern::compile(pattern, to_lower);

                assert_eq!(
                    compiled.matches(value),
                    general.matches(value),
                    "{pattern:?} {value:?} to_lower={to_lower}",
                );

                let mut shape_captures = Vec::new();
                let mut general_captures = Vec::new();
                assert_eq!(
                    compiled.capture(value, u64::MAX, &mut shape_captures),
                    general.capture(value, u64::MAX, &mut general_captures),
                    "{pattern:?} {value:?} to_lower={to_lower}",
                );
                assert_eq!(
                    shape_captures, general_captures,
                    "{pattern:?} {value:?} to_lower={to_lower}",
                );
            }
        }
    }
}
