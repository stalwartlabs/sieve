/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::char::REPLACEMENT_CHARACTER;

use crate::MAX_MATCH_VARIABLES;

#[derive(Debug)]
enum PatternChar {
    WildcardMany { num: usize, match_pos: usize },
    WildcardSingle { match_pos: usize },
    Char { char: char, match_pos: usize },
}

fn compile(str: &str, to_lower: bool) -> Vec<PatternChar> {
    let mut chars = Vec::new();
    let mut is_escaped = false;
    let mut str = str.chars().peekable();

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
    chars
}

// Credits: Algorithm ported from https://research.swtch.com/glob

pub(crate) fn glob_match(value: &str, pattern: &str, to_lower: bool) -> bool {
    let pattern = compile(pattern, to_lower);
    let value = if to_lower {
        value.to_lowercase().chars().collect::<Vec<_>>()
    } else {
        value.chars().collect::<Vec<_>>()
    };

    let mut px = 0;
    let mut nx = 0;
    let mut next_px = 0;
    let mut next_nx = 0;

    while px < pattern.len() || nx < value.len() {
        match pattern.get(px) {
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

pub(crate) fn glob_match_capture(
    value_: &str,
    pattern: &str,
    to_lower: bool,
    capture_positions: u64,
    captured_values: &mut Vec<(usize, String)>,
) -> bool {
    let mut pattern = compile(pattern, to_lower);
    let value = if to_lower {
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

    while px < pattern.len() || nx < value.len() {
        match pattern.get_mut(px) {
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

    let mut wildcard_pos = 1;
    for item in pattern {
        if wildcard_pos <= MAX_MATCH_VARIABLES {
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
                PatternChar::WildcardSingle { match_pos } => {
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
                PatternChar::Char { match_pos, .. } => match_pos,
            } + 1;
        } else {
            break;
        }
    }
    true
}

#[cfg(test)]
mod tests {
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
            let mut match_values = Vec::new();
            assert!(
                super::glob_match_capture(value, pattern, true, u64::MAX ^ 1, &mut match_values),
                "{value:?} {pattern:?}",
            );

            assert_eq!(
                match_values.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
                expected_result,
                "{value:?} {pattern:?}",
            );
            assert!(
                super::glob_match(value, pattern, true),
                "{value:?} {pattern:?}",
            );
        }
    }
}
