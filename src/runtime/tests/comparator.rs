use regex::Regex;

use crate::{
    compiler::grammar::{Comparator, RelationalMatch},
    MatchAs,
};

use super::glob::{glob_match, glob_match_capture};

impl Comparator {
    pub(crate) fn is(&self, a: &str, b: &str) -> bool {
        match self {
            Comparator::Octet => a == b,
            Comparator::AsciiNumeric => a.parse::<f64>() == b.parse::<f64>(),
            _ => a.to_lowercase() == b.to_lowercase(),
        }
    }

    pub(crate) fn contains(&self, haystack: &str, needle: &str) -> bool {
        needle.is_empty()
            || match self {
                Comparator::Octet => haystack.contains(needle),
                _ => haystack.to_lowercase().contains(&needle.to_lowercase()),
            }
    }

    pub(crate) fn relational(&self, relation: &RelationalMatch, a: &str, b: &str) -> bool {
        match self {
            Comparator::Octet => relation.cmp(a, b.as_ref()),
            Comparator::AsciiNumeric => relation.cmp(
                &a.parse::<f64>().unwrap_or(f64::MAX),
                &b.parse::<f64>().unwrap_or(f64::MAX),
            ),
            _ => relation.cmp(&a.to_lowercase(), &b.to_lowercase()),
        }
    }

    pub(crate) fn matches(
        &self,
        value: &str,
        pattern: &str,
        capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        match self {
            Comparator::AsciiCaseMap if capture_positions == 0 => {
                glob_match(&value.to_lowercase(), pattern, true)
            }
            Comparator::AsciiCaseMap => {
                glob_match_capture(value, pattern, true, capture_positions, captured_values)
            }
            _ if capture_positions == 0 => glob_match(value, pattern, false),
            _ => glob_match_capture(value, pattern, false, capture_positions, captured_values),
        }
    }

    pub(crate) fn regex(
        &self,
        value: &str,
        pattern: &str,
        mut capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        match Regex::new(pattern) {
            Ok(re) => {
                //TODO cache compilation
                if capture_positions == 0 {
                    re.is_match(value)
                } else if let Some(captures) = re.captures(value) {
                    captured_values.clear();
                    while capture_positions != 0 {
                        let index = 63 - capture_positions.leading_zeros();
                        capture_positions ^= 1 << index;
                        if let Some(match_var) = captures.get(index as usize) {
                            captured_values.push((index as usize, match_var.as_str().to_string()));
                        }
                    }
                    true
                } else {
                    false
                }
            }
            Err(err) => {
                debug_assert!(false, "Failed to compile regex: {:?}", err);
                false
            }
        }
    }

    pub(crate) fn as_match(&self) -> MatchAs {
        match self {
            Comparator::AsciiCaseMap => MatchAs::Lowercase,
            Comparator::AsciiNumeric => MatchAs::Number,
            _ => MatchAs::Octet,
        }
    }
}

impl RelationalMatch {
    pub(crate) fn cmp_num(&self, num: f64, value: &str) -> bool {
        if let Ok(value) = value.parse::<f64>() {
            self.cmp(&num, &value)
        } else {
            false
        }
    }

    pub(crate) fn cmp<T>(&self, haystack: &T, needle: &T) -> bool
    where
        T: PartialOrd + ?Sized,
    {
        match self {
            RelationalMatch::Gt => haystack.gt(needle),
            RelationalMatch::Ge => haystack.ge(needle),
            RelationalMatch::Lt => haystack.lt(needle),
            RelationalMatch::Le => haystack.le(needle),
            RelationalMatch::Eq => haystack.eq(needle),
            RelationalMatch::Ne => haystack.ne(needle),
        }
    }
}
