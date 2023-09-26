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

use std::borrow::Cow;

use crate::{
    compiler::{
        grammar::{Comparator, RelationalMatch},
        Value,
    },
    runtime::Variable,
    MatchAs,
};

use super::glob::GlobPattern;

impl Comparator {
    pub(crate) fn is(&self, a: &Variable<'_>, b: &Variable<'_>) -> bool {
        match self {
            Comparator::Octet => a.to_cow() == b.to_cow(),
            Comparator::AsciiNumeric => RelationalMatch::Eq.cmp(&a.to_number(), &b.to_number()),
            _ => a.to_cow().to_lowercase() == b.to_cow().to_lowercase(),
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
        a: &Variable<'_>,
        b: &Variable<'_>,
    ) -> bool {
        match self {
            Comparator::Octet => relation.cmp(a.to_cow().as_ref(), b.to_cow().as_ref()),
            Comparator::AsciiNumeric => relation.cmp(&a.to_number(), &b.to_number()),
            _ => relation.cmp(&a.to_cow().to_lowercase(), &b.to_cow().to_lowercase()),
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
        pattern_expr: &Variable<'_>,
        value: &str,
        mut capture_positions: u64,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        let regex = if let Value::Regex(regex) = pattern {
            Cow::Borrowed(&regex.regex)
        } else {
            match fancy_regex::Regex::new(pattern_expr.to_cow().as_ref()) {
                Ok(regex) => Cow::Owned(regex),
                Err(err) => {
                    debug_assert!(false, "Failed to compile regex: {err:?}");
                    return false;
                }
            }
        };

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

    pub(crate) fn as_match(&self) -> MatchAs {
        match self {
            Comparator::AsciiCaseMap => MatchAs::Lowercase,
            Comparator::AsciiNumeric => MatchAs::Number,
            _ => MatchAs::Octet,
        }
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
