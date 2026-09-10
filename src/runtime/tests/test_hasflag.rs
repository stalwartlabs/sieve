/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{TestResult, glob::CompiledGlob};
use crate::{
    Context, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::{RuntimeError, Variable, eval::ValueRef},
};
use smallvec::SmallVec;

impl<'x> Context<'x> {
    pub(crate) fn test_hasflag(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestHasFlag,
    ) -> Result<TestResult, RuntimeError> {
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();
        let use_global_flags = test.variable_list.is_empty();
        let mut variables: SmallVec<[Variable<'x>; 4]> = SmallVec::new();
        let mut iter = script.recs(test.variable_list)?;
        while let Some(rec) = iter.next() {
            let value = ValueRef::decode(script, rec, &mut iter)?;
            if let Some(flags) = self.variable_ref(script, value)?
                && !flags.is_empty()
            {
                variables.push(flags);
            }
        }

        let result = if let MatchType::Count(rel_match) = &match_type {
            let flag_count = if use_global_flags {
                self.global_flags().len()
            } else {
                variables
                    .iter()
                    .map(|flags| flags.to_string().split(' ').count())
                    .sum()
            };

            self.eval_values(script, test.flags)?
                .iter()
                .any(|key| rel_match.cmp(&Number::from(flag_count as i64), &key.to_number()))
        } else {
            let mut captured_values = Vec::new();
            let result = self.tokenize_flags(script, test.flags, |check_flag| {
                if use_global_flags {
                    self.global_flags().iter().any(|flag| {
                        check_flag_matches(
                            &comparator,
                            &match_type,
                            flag,
                            check_flag,
                            &mut captured_values,
                        )
                    })
                } else {
                    variables.iter().any(|flags| {
                        flags.to_string().split(' ').any(|flag| {
                            check_flag_matches(
                                &comparator,
                                &match_type,
                                flag,
                                check_flag,
                                &mut captured_values,
                            )
                        })
                    })
                }
            })?;
            if !captured_values.is_empty() {
                self.set_match_variables(captured_values);
            }
            result
        };

        Ok(TestResult::Bool(result ^ test.is_not))
    }
}

fn check_flag_matches(
    comparator: &Comparator,
    match_type: &MatchType,
    flag: &str,
    check_flag: &str,
    captured_values: &mut Vec<(usize, String)>,
) -> bool {
    match match_type {
        MatchType::Is => comparator.is(&flag, &check_flag),
        MatchType::Contains => comparator.contains(flag, check_flag),
        MatchType::Value(rel_match) => comparator.relational(rel_match, &flag, &check_flag),
        MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
            let glob = CompiledGlob::compile(check_flag, comparator.is_casemap());
            if *capture_positions == 0 {
                glob.matches(flag)
            } else {
                glob.capture(flag, *capture_positions, captured_values)
            }
        }
        MatchType::Count(_) | MatchType::List => false,
    }
}
