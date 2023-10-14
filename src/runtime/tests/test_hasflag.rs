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
        grammar::{tests::test_hasflag::TestHasFlag, MatchType},
        Number, VariableType,
    },
    Context,
};

use super::TestResult;

impl TestHasFlag {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let mut variable_list_ = None;
        let variable_list = if !self.variable_list.is_empty() {
            &self.variable_list
        } else {
            variable_list_.get_or_insert_with(|| vec![VariableType::Global("__flags".to_string())])
        };

        let result = if let MatchType::Count(rel_match) = &self.match_type {
            let mut flag_count = 0;
            for variable in variable_list {
                match ctx.get_variable(variable) {
                    Some(flags) if !flags.is_empty() => {
                        flag_count += flags.to_string().split(' ').count();
                    }
                    _ => (),
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
                for variable in variable_list {
                    match ctx.get_variable(variable) {
                        Some(flags) if !flags.is_empty() => {
                            for flag in flags.to_string().split(' ') {
                                if match &self.match_type {
                                    MatchType::Is => self.comparator.is(&flag, &check_flag),
                                    MatchType::Contains => {
                                        self.comparator.contains(flag, check_flag)
                                    }
                                    MatchType::Value(rel_match) => {
                                        self.comparator.relational(rel_match, &flag, &check_flag)
                                    }
                                    MatchType::Matches(capture_positions) => {
                                        self.comparator.matches(
                                            flag,
                                            check_flag,
                                            *capture_positions,
                                            &mut captured_values,
                                        )
                                    }
                                    MatchType::Regex(capture_positions) => self.comparator.matches(
                                        flag,
                                        check_flag,
                                        *capture_positions,
                                        &mut captured_values,
                                    ),
                                    MatchType::Count(_) | MatchType::List => false,
                                } {
                                    return true;
                                }
                            }
                        }
                        _ => (),
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
}
