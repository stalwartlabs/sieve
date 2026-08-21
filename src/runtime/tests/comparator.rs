/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::{borrow::Cow, cmp::Ordering, sync::Arc};

use crate::{
    MatchAs,
    compiler::{
        Number, Value,
        grammar::{Comparator, RelationalMatch},
    },
    runtime::Variable,
};

use super::glob::CompiledGlob;

pub(crate) trait Comparable {
    fn to_str(&'_ self) -> Cow<'_, str>;
    fn to_number(&self) -> Number;
}

impl Comparator {
    pub(crate) fn is(&self, a: &impl Comparable, b: &impl Comparable) -> bool {
        match self {
            Comparator::Octet => a.to_str() == b.to_str(),
            Comparator::AsciiNumeric => RelationalMatch::Eq.cmp(&a.to_number(), &b.to_number()),
            _ => casemap_eq(a.to_str().as_ref(), b.to_str().as_ref()),
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
            _ => relation.matches(casemap_cmp(a.to_str().as_ref(), b.to_str().as_ref())),
        }
    }

    pub(crate) fn matches(
        &self,
        pattern: Option<&Value>,
        pattern_expr: &str,
        value: &str,
        capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        let to_lower = matches!(self, Comparator::AsciiCaseMap);

        if let Some(Value::Glob(glob)) = pattern {
            let cached = glob.glob.0.load();
            if let Some(compiled) = cached.as_ref() {
                eval_glob(compiled, value, capture_positions, captured_values)
            } else {
                let compiled = CompiledGlob::compile(&glob.expr, to_lower);
                let result = eval_glob(&compiled, value, capture_positions, captured_values);
                glob.glob.0.store(Arc::new(Some(compiled)));
                result
            }
        } else {
            let compiled = CompiledGlob::compile(pattern_expr, to_lower);
            eval_glob(&compiled, value, capture_positions, captured_values)
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

fn eval_glob(
    compiled: &CompiledGlob,
    value: &str,
    capture_positions: u64,
    captured_values: &mut Vec<(usize, String)>,
) -> bool {
    if capture_positions == 0 {
        compiled.matches(value)
    } else {
        compiled.capture(value, capture_positions, captured_values)
    }
}

pub(crate) fn casemap_eq(a: &str, b: &str) -> bool {
    if a.is_ascii() && b.is_ascii() {
        a.eq_ignore_ascii_case(b)
    } else {
        a.to_lowercase() == b.to_lowercase()
    }
}

pub(crate) fn casemap_cmp(a: &str, b: &str) -> Ordering {
    if a.is_ascii() && b.is_ascii() {
        a.bytes()
            .map(|c| c.to_ascii_lowercase())
            .cmp(b.bytes().map(|c| c.to_ascii_lowercase()))
    } else {
        a.to_lowercase().cmp(&b.to_lowercase())
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
    fn to_str(&'_ self) -> Cow<'_, str> {
        self.to_string()
    }

    fn to_number(&self) -> Number {
        self.to_number()
    }
}

impl Comparable for &str {
    fn to_str(&'_ self) -> Cow<'_, str> {
        (*self).into()
    }

    fn to_number(&self) -> Number {
        self.parse::<f64>()
            .map(Number::Float)
            .unwrap_or(Number::Float(0.0))
    }
}

impl RelationalMatch {
    pub fn matches(&self, ordering: Ordering) -> bool {
        match self {
            RelationalMatch::Gt => ordering == Ordering::Greater,
            RelationalMatch::Ge => ordering != Ordering::Less,
            RelationalMatch::Lt => ordering == Ordering::Less,
            RelationalMatch::Le => ordering != Ordering::Greater,
            RelationalMatch::Eq => ordering == Ordering::Equal,
            RelationalMatch::Ne => ordering != Ordering::Equal,
        }
    }

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
