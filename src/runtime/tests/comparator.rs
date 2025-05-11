/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::{borrow::Cow, sync::Arc};

use crate::{
    compiler::{
        grammar::{Comparator, RelationalMatch},
        Number, Value,
    },
    runtime::Variable,
    MatchAs,
};

use super::glob::GlobPattern;

pub(crate) trait Comparable {
    fn to_str(&self) -> Cow<str>;
    fn to_number(&self) -> Number;
}

impl Comparator {
    pub(crate) fn is(&self, a: &impl Comparable, b: &impl Comparable) -> bool {
        match self {
            Comparator::Octet => a.to_str() == b.to_str(),
            Comparator::AsciiNumeric => RelationalMatch::Eq.cmp(&a.to_number(), &b.to_number()),
            _ => a.to_str().to_lowercase() == b.to_str().to_lowercase(),
        }
    }

    pub(crate) fn contains(&self, haystack: &str, needle: &str) -> bool {
        needle.is_empty()
            || match self {
                Comparator::Octet => haystack.contains(needle),
                _ => haystack.to_lowercase().contains(&needle.to_lowercase()),
            }
    }

    pub(crate) fn relational(
        &self,
        relation: &RelationalMatch,
        a: &impl Comparable,
        b: &impl Comparable,
    ) -> bool {
        match self {
            Comparator::Octet => relation.cmp(a.to_str().as_ref(), b.to_str().as_ref()),
            Comparator::AsciiNumeric => relation.cmp(&a.to_number(), &b.to_number()),
            _ => relation.cmp(&a.to_str().to_lowercase(), &b.to_str().to_lowercase()),
        }
    }

    pub(crate) fn matches(
        &self,
        value: &str,
        pattern: &str,
        capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        let pattern = GlobPattern::compile(pattern, matches!(self, Comparator::AsciiCaseMap));
        match self {
            Comparator::AsciiCaseMap if capture_positions == 0 => pattern.matches(value),
            Comparator::AsciiCaseMap => pattern.capture(value, capture_positions, captured_values),
            _ if capture_positions == 0 => pattern.matches(value),
            _ => pattern.capture(value, capture_positions, captured_values),
        }
    }

    pub(crate) fn regex(
        &self,
        pattern: &Value,
        pattern_expr: &Variable,
        value: &str,
        capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        if let Value::Regex(regex) = pattern {
            let lazy_regex = regex.regex.0.load();
            if let Some(regex) = lazy_regex.as_ref() {
                eval_regex(regex, value, capture_positions, captured_values)
            } else {
                match fancy_regex::Regex::new(&regex.expr) {
                    Ok(fancy_regex) => {
                        let result =
                            eval_regex(&fancy_regex, value, capture_positions, captured_values);
                        regex.regex.0.store(Arc::new(Some(fancy_regex)));
                        result
                    }
                    Err(err) => {
                        debug_assert!(false, "Failed to compile regex: {err:?}");
                        false
                    }
                }
            }
        } else {
            match fancy_regex::Regex::new(pattern_expr.to_string().as_ref()) {
                Ok(regex) => eval_regex(&regex, value, capture_positions, captured_values),
                Err(err) => {
                    debug_assert!(false, "Failed to compile regex: {err:?}");
                    false
                }
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

fn eval_regex(
    regex: &fancy_regex::Regex,
    value: &str,
    mut capture_positions: u64,
    captured_values: &mut Vec<(usize, String)>,
) -> bool {
    if capture_positions == 0 {
        regex.is_match(value).unwrap_or_default()
    } else if let Ok(Some(captures)) = regex.captures(value) {
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

impl Comparable for Variable {
    fn to_str(&self) -> Cow<str> {
        self.to_string()
    }

    fn to_number(&self) -> Number {
        self.to_number()
    }
}

impl Comparable for &str {
    fn to_str(&self) -> Cow<str> {
        (*self).into()
    }

    fn to_number(&self) -> Number {
        self.parse::<f64>()
            .map(Number::Float)
            .unwrap_or(Number::Float(0.0))
    }
}

impl RelationalMatch {
    pub fn cmp<T>(&self, a: &T, b: &T) -> bool
    where
        T: PartialOrd + ?Sized,
    {
        match self {
            RelationalMatch::Gt => a.gt(b),
            RelationalMatch::Ge => a.ge(b),
            RelationalMatch::Lt => a.lt(b),
            RelationalMatch::Le => a.le(b),
            RelationalMatch::Eq => a.eq(b),
            RelationalMatch::Ne => a.ne(b),
        }
    }
}
