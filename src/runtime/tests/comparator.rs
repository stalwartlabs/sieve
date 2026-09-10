/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    MatchAs,
    compiler::{
        Number,
        grammar::{Comparator, RelationalMatch},
    },
    runtime::Variable,
};
use std::{borrow::Cow, cmp::Ordering};

pub(crate) trait Comparable {
    fn to_str(&'_ self) -> Cow<'_, str>;
    fn to_number(&self) -> Number;
}

impl Comparator {
    #[inline(always)]
    pub(crate) fn from_code(code: u8) -> Comparator {
        match code {
            0 => Comparator::Elbonia,
            2 => Comparator::AsciiCaseMap,
            3 => Comparator::AsciiNumeric,
            _ => Comparator::Octet,
        }
    }

    #[inline(always)]
    pub(crate) fn code(&self) -> u8 {
        match self {
            Comparator::Elbonia | Comparator::Other(_) => 0,
            Comparator::Octet => 1,
            Comparator::AsciiCaseMap => 2,
            Comparator::AsciiNumeric => 3,
        }
    }

    #[inline(always)]
    pub(crate) fn is_casemap(&self) -> bool {
        matches!(self, Comparator::AsciiCaseMap)
    }

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
                _ => contains_ignore_ascii_case(haystack, needle),
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

    pub(crate) fn as_match(&self) -> MatchAs {
        match self {
            Comparator::AsciiCaseMap => MatchAs::Lowercase,
            Comparator::AsciiNumeric => MatchAs::Number,
            _ => MatchAs::Octet,
        }
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

pub(crate) fn contains_ignore_ascii_case(haystack: &str, needle: &str) -> bool {
    if needle.is_empty() {
        return true;
    }
    if !needle.is_ascii() || !haystack.is_ascii() {
        return haystack.to_lowercase().contains(&needle.to_lowercase());
    }
    let haystack = haystack.as_bytes();
    let needle = needle.as_bytes();
    if needle.len() > haystack.len() {
        return false;
    }
    let first = needle[0];
    let lower = first.to_ascii_lowercase();
    let upper = first.to_ascii_uppercase();
    let last_start = haystack.len() - needle.len();
    let mut offset = 0;
    while offset <= last_start {
        let window = &haystack[offset..=last_start];
        let found = if lower != upper {
            memchr::memchr2(lower, upper, window)
        } else {
            memchr::memchr(first, window)
        };
        let Some(at) = found else {
            return false;
        };
        let start = offset + at;
        if haystack[start..start + needle.len()].eq_ignore_ascii_case(needle) {
            return true;
        }
        offset = start + 1;
    }
    false
}

pub(crate) fn starts_with_ignore_ascii_case(value: &str, prefix: &str) -> bool {
    if value.is_ascii() && prefix.is_ascii() {
        value.len() >= prefix.len()
            && value.as_bytes()[..prefix.len()].eq_ignore_ascii_case(prefix.as_bytes())
    } else {
        value.to_lowercase().starts_with(&prefix.to_lowercase())
    }
}

pub(crate) fn ends_with_ignore_ascii_case(value: &str, suffix: &str) -> bool {
    if value.is_ascii() && suffix.is_ascii() {
        value.len() >= suffix.len()
            && value.as_bytes()[value.len() - suffix.len()..]
                .eq_ignore_ascii_case(suffix.as_bytes())
    } else {
        value.to_lowercase().ends_with(&suffix.to_lowercase())
    }
}

impl Comparable for Variable<'_> {
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

impl Comparable for str {
    fn to_str(&'_ self) -> Cow<'_, str> {
        self.into()
    }

    fn to_number(&self) -> Number {
        self.parse::<f64>()
            .map(Number::Float)
            .unwrap_or(Number::Float(0.0))
    }
}

impl RelationalMatch {
    #[inline(always)]
    pub(crate) fn from_code(code: u64) -> RelationalMatch {
        match code {
            0 => RelationalMatch::Gt,
            1 => RelationalMatch::Ge,
            2 => RelationalMatch::Lt,
            3 => RelationalMatch::Le,
            4 => RelationalMatch::Eq,
            _ => RelationalMatch::Ne,
        }
    }

    #[inline(always)]
    pub(crate) fn code(&self) -> u64 {
        match self {
            RelationalMatch::Gt => 0,
            RelationalMatch::Ge => 1,
            RelationalMatch::Lt => 2,
            RelationalMatch::Le => 3,
            RelationalMatch::Eq => 4,
            RelationalMatch::Ne => 5,
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_comparators_fold_case() {
        let other = Comparator::from_code(Comparator::Other("i;unicode-casemap".into()).code());
        assert!(other.is(&"HELLO", &"hello"));
        assert!(other.contains("This is a TEST header", "test"));
        assert!(other.relational(&RelationalMatch::Eq, &"HELLO", &"hello"));
        assert!(!other.is_casemap());
        let octet = Comparator::from_code(Comparator::Octet.code());
        assert!(!octet.is(&"HELLO", &"hello"));
        assert!(!octet.contains("This is a TEST header", "test"));
    }

    #[test]
    fn ascii_case_insensitive_search() {
        for (haystack, needle, expected) in [
            ("Hello World", "world", true),
            ("Hello World", "WORLD", true),
            ("Hello World", "o w", true),
            ("Hello World", "xyz", false),
            ("Hello World", "", true),
            ("", "a", false),
            ("aaa", "aaaa", false),
            ("aAaAb", "aab", true),
            ("...", ".", true),
            ("Grüße", "grüsse", false),
            ("Grüße", "ÜSSE", false),
            ("Grüße", "grüß", true),
        ] {
            assert_eq!(
                contains_ignore_ascii_case(haystack, needle),
                expected,
                "{haystack:?} contains {needle:?}"
            );
            assert_eq!(
                haystack.to_lowercase().contains(&needle.to_lowercase()),
                expected,
                "reference {haystack:?} contains {needle:?}"
            );
        }
    }
}
