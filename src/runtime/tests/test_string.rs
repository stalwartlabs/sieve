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

use crate::{
    compiler::{
        grammar::{tests::test_string::TestString, MatchType},
        Number,
    },
    Context, Event,
};

use super::TestResult;

impl TestString {
    pub(crate) fn exec(&self, ctx: &mut Context, empty_is_null: bool) -> TestResult {
        let mut result = false;

        match &self.match_type {
            MatchType::Count(match_type) => {
                let num_items = self
                    .source
                    .iter()
                    .filter(|x| !ctx.eval_value(x).is_empty())
                    .count() as i64;
                if !empty_is_null || num_items > 0 {
                    for key in &self.key_list {
                        if match_type
                            .cmp(&Number::from(num_items), &ctx.eval_value(key).to_number())
                        {
                            result = true;
                            break;
                        }
                    }
                }
            }
            MatchType::List => {
                let mut values = Vec::with_capacity(self.source.len());
                for source in &self.source {
                    let value = ctx.eval_value(source).into_cow();
                    if !value.is_empty() && !values.iter().any(|v: &String| v.eq(value.as_ref())) {
                        values.push(value.into_owned());
                    }
                }
                if !values.is_empty() {
                    return TestResult::Event {
                        event: Event::ListContains {
                            lists: ctx.eval_values_owned(&self.key_list),
                            values,
                            match_as: self.comparator.as_match(),
                        },
                        is_not: self.is_not,
                    };
                }
            }
            _ => {
                let mut captured_values = Vec::new();
                let sources = ctx.eval_values(&self.source);

                for key in &self.key_list {
                    let key = ctx.eval_value(key);
                    for source in &sources {
                        if !empty_is_null || !source.is_empty() {
                            result = match &self.match_type {
                                MatchType::Is => self.comparator.is(source, &key),
                                MatchType::Contains => self
                                    .comparator
                                    .contains(source.to_cow().as_ref(), key.to_cow().as_ref()),
                                MatchType::Value(relation) => {
                                    self.comparator.relational(relation, source, &key)
                                }
                                MatchType::Matches(capture_positions) => self.comparator.matches(
                                    source.to_cow().as_ref(),
                                    key.to_cow().as_ref(),
                                    *capture_positions,
                                    &mut captured_values,
                                ),
                                MatchType::Regex(capture_positions) => self.comparator.regex(
                                    source.to_cow().as_ref(),
                                    key.to_cow().as_ref(),
                                    *capture_positions,
                                    &mut captured_values,
                                ),
                                _ => false,
                            };

                            if result {
                                break;
                            }
                        }
                    }
                }

                if !captured_values.is_empty() {
                    ctx.set_match_variables(captured_values);
                }
            }
        }

        TestResult::Bool(result ^ self.is_not)
    }
}
