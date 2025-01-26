/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
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
                    let value = ctx.eval_value(source).to_string().into_owned();
                    if !value.is_empty() && !values.iter().any(|v: &String| v.eq(&value)) {
                        values.push(value);
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

                for pattern in &self.key_list {
                    let key = ctx.eval_value(pattern);
                    for source in &sources {
                        if !empty_is_null || !source.is_empty() {
                            result = match &self.match_type {
                                MatchType::Is => self.comparator.is(source, &key),
                                MatchType::Contains => self.comparator.contains(
                                    source.to_string().as_ref(),
                                    key.to_string().as_ref(),
                                ),
                                MatchType::Value(relation) => {
                                    self.comparator.relational(relation, source, &key)
                                }
                                MatchType::Matches(capture_positions) => self.comparator.matches(
                                    source.to_string().as_ref(),
                                    key.to_string().as_ref(),
                                    *capture_positions,
                                    &mut captured_values,
                                ),
                                MatchType::Regex(capture_positions) => self.comparator.regex(
                                    pattern,
                                    &key,
                                    source.to_string().as_ref(),
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
