/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{
    Context,
    compiler::{
        Number,
        grammar::{MatchType, tests::test_hasflag::TestHasFlag},
    },
};

impl TestHasFlag {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let variable_list = &self.variable_list;

        let result = if let MatchType::Count(rel_match) = &self.match_type {
            let mut flag_count = 0;
            if variable_list.is_empty() {
                flag_count = ctx.global_flags().len();
            } else {
                for variable in variable_list {
                    match ctx.get_variable(variable) {
                        Some(flags) if !flags.is_empty() => {
                            flag_count += flags.to_string().split(' ').count();
                        }
                        _ => (),
                    }
                }
            }

            let mut result = false;
            for key in &self.flags {
                if rel_match.cmp(
                    &Number::from(flag_count as i64),
                    &ctx.eval_value(key).to_number(),
                ) {
                    result = true;
                    break;
                }
            }
            result
        } else {
            let mut captured_values = Vec::new();
            let result = ctx.tokenize_flags(&self.flags, |check_flag| {
                if variable_list.is_empty() {
                    for flag in ctx.global_flags() {
                        if self.check_flag(flag, check_flag, &mut captured_values) {
                            return true;
                        }
                    }
                } else {
                    for variable in variable_list {
                        match ctx.get_variable(variable) {
                            Some(flags) if !flags.is_empty() => {
                                for flag in flags.to_string().split(' ') {
                                    if self.check_flag(flag, check_flag, &mut captured_values) {
                                        return true;
                                    }
                                }
                            }
                            _ => (),
                        }
                    }
                }
                false
            });
            if !captured_values.is_empty() {
                ctx.set_match_variables(captured_values);
            }
            result
        };

        TestResult::Bool(result ^ self.is_not)
    }

    fn check_flag(
        &self,
        flag: &str,
        check_flag: &str,
        captured_values: &mut Vec<(usize, String)>,
    ) -> bool {
        match &self.match_type {
            MatchType::Is => self.comparator.is(&flag, &check_flag),
            MatchType::Contains => self.comparator.contains(flag, check_flag),
            MatchType::Value(rel_match) => {
                self.comparator.relational(rel_match, &flag, &check_flag)
            }
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => self
                .comparator
                .matches(None, check_flag, flag, *capture_positions, captured_values),
            MatchType::Count(_) | MatchType::List => false,
        }
    }
}
