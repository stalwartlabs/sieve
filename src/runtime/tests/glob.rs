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

pub(crate) fn glob_match_with_values(
    value_: &str,
    pattern: &str,
    to_lower: bool,
    positions: u64,
    matched_values: &mut Vec<(usize, String)>,
) -> bool {
    let mut pattern = compile(pattern, to_lower);
    let value = if to_lower {
        value_.to_lowercase().chars().collect::<Vec<_>>()
    } else {
        value_.chars().collect::<Vec<_>>()
    };

    let mut px = 0;
    let mut nx = 0;
    let mut next_px = 0;
    let mut next_nx = 0;

    while px < pattern.len() || nx < value.len() {
        match pattern.get_mut(px) {
            Some(PatternChar::Char { char, match_pos }) => {
                if matches!(value.get(nx), Some(nc) if nc == char ) {
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

    matched_values.clear();
    if positions & 1 != 0 {
        matched_values.push((0usize, value_.to_string()));
    }

    let mut wildcard_pos = 1;
    for item in pattern {
        if wildcard_pos <= MAX_MATCH_VARIABLES {
            last_pos = match item {
                PatternChar::WildcardMany { mut num, match_pos } => {
                    if positions & (1 << wildcard_pos) != 0 {
                        if let Some(range) = value.get(last_pos..match_pos) {
                            matched_values.push((wildcard_pos, range.iter().collect::<String>()));
                        } else {
                            debug_assert!(false, "Glob pattern failure.");
                            return false;
                        }
                    }
                    num -= 1;
                    wildcard_pos += 1;
                    while num > 0 {
                        if positions & (1 << wildcard_pos) != 0 {
                            matched_values.push((wildcard_pos, String::with_capacity(0)));
                        }
                        wildcard_pos += 1;
                        num -= 1;
                    }
                    match_pos
                }
                PatternChar::WildcardSingle { match_pos } => {
                    if positions & (1 << wildcard_pos) != 0 {
                        if let Some(char) = value.get(match_pos) {
                            matched_values.push((wildcard_pos, char.to_string()));
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
        ] {
            let mut match_values = Vec::new();
            assert!(
                super::glob_match_with_values(
                    value,
                    pattern,
                    true,
                    u64::MAX ^ 1,
                    &mut match_values
                ),
                "{:?} {:?}",
                value,
                pattern
            );

            assert_eq!(
                match_values.into_iter().map(|(_, v)| v).collect::<Vec<_>>(),
                expected_result,
                "{:?} {:?}",
                value,
                pattern
            );
            assert!(
                super::glob_match(value, pattern, true),
                "{:?} {:?}",
                value,
                pattern
            );
        }
    }
}
