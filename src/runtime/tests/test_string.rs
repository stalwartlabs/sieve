/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{
    Context, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::{RuntimeError, handler::Handler},
};
use smallvec::SmallVec;

impl<'x> Context<'x> {
    pub(crate) fn test_environment<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestEnvironment,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let test = ops::TestString {
            match_type: test.match_type,
            comparator: test.comparator,
            source: test.source,
            key_list: test.key_list,
            is_not: test.is_not,
        };
        self.test_string(script, &test, true, handler)
    }

    pub(crate) fn test_string<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestString,
        empty_is_null: bool,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let mut result = false;
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();

        match &match_type {
            MatchType::Count(rel) => {
                let sources = self.eval_values(script, test.source)?;
                let num_items = sources.iter().filter(|x| !x.is_empty()).count() as i64;
                if !empty_is_null || num_items > 0 {
                    for key in self.eval_values(script, test.key_list)? {
                        if rel.cmp(&Number::from(num_items), &key.to_number()) {
                            result = true;
                            break;
                        }
                    }
                }
            }
            MatchType::List => {
                let sources = self.eval_values(script, test.source)?;
                let mut values: SmallVec<[&str; 4]> = SmallVec::with_capacity(sources.len());
                for source in &sources {
                    let value = self.intern_cow(source.clone().into_string());
                    if !value.is_empty() && !values.contains(&value) {
                        values.push(value);
                    }
                }
                if !values.is_empty() {
                    let lists = self.eval_strings(script, test.key_list)?;
                    return TestResult::from_reply(
                        handler.list_contains(self, &lists, &values, comparator.as_match()),
                        test.is_not,
                    );
                }
            }
            _ => {
                let mut captured_values = Vec::new();
                let sources = self.eval_values(script, test.source)?;
                let keys = self.eval_keys(script, test.key_list)?;

                'outer: for key in &keys {
                    for source in &sources {
                        if !empty_is_null || !source.is_empty() {
                            result = match &match_type {
                                MatchType::Is => comparator.is(source, &key.value),
                                MatchType::Contains => comparator.contains(
                                    source.to_string().as_ref(),
                                    key.value.to_string().as_ref(),
                                ),
                                MatchType::Value(relation) => {
                                    comparator.relational(relation, source, &key.value)
                                }
                                MatchType::Matches(capture_positions) => self.glob_matches(
                                    script,
                                    comparator.is_casemap(),
                                    key,
                                    source.to_string().as_ref(),
                                    *capture_positions,
                                    &mut captured_values,
                                )?,
                                MatchType::Regex(capture_positions) => self.regex_matches(
                                    script,
                                    key,
                                    source.to_string().as_ref(),
                                    *capture_positions,
                                    &mut captured_values,
                                )?,
                                _ => false,
                            };

                            if result {
                                break 'outer;
                            }
                        }
                    }
                }

                if !captured_values.is_empty() {
                    self.set_match_variables(captured_values);
                }
            }
        }

        Ok(TestResult::Bool(result ^ test.is_not))
    }
}
