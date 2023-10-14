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
        grammar::{
            tests::test_spamtest::{TestSpamTest, TestVirusTest},
            MatchType,
        },
        Number,
    },
    runtime::Variable,
    Context, SpamStatus, VirusStatus,
};

use super::TestResult;

impl TestSpamTest {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let status = if self.percent {
            ctx.spam_status.as_percentage()
        } else {
            ctx.spam_status.as_number()
        };
        let value = ctx.eval_value(&self.value);
        let mut captured_values = Vec::new();

        let result = match &self.match_type {
            MatchType::Is => self.comparator.is(&status, &value),
            MatchType::Contains => self
                .comparator
                .contains(status.to_string().as_ref(), value.to_string().as_ref()),
            MatchType::Value(rel_match) => self.comparator.relational(rel_match, &status, &value),
            MatchType::Matches(capture_positions) => self.comparator.matches(
                status.to_string().as_ref(),
                value.to_string().as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Regex(capture_positions) => self.comparator.regex(
                &self.value,
                &value,
                status.to_string().as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Count(rel_match) => rel_match.cmp(
                &Number::from(if matches!(&ctx.spam_status, SpamStatus::Unknown) {
                    0.0
                } else {
                    1.1
                }),
                &value.to_number(),
            ),
            MatchType::List => false,
        };

        if !captured_values.is_empty() {
            ctx.set_match_variables(captured_values);
        }

        TestResult::Bool(result ^ self.is_not)
    }
}

impl TestVirusTest {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let status = ctx.virus_status.as_number();
        let value = ctx.eval_value(&self.value);
        let mut captured_values = Vec::new();

        let result = match &self.match_type {
            MatchType::Is => self.comparator.is(&status, &value),
            MatchType::Contains => self
                .comparator
                .contains(status.to_string().as_ref(), value.to_string().as_ref()),
            MatchType::Value(rel_match) => self.comparator.relational(rel_match, &status, &value),
            MatchType::Matches(capture_positions) => self.comparator.matches(
                status.to_string().as_ref(),
                value.to_string().as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Regex(capture_positions) => self.comparator.regex(
                &self.value,
                &value,
                status.to_string().as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Count(rel_match) => rel_match.cmp(
                &Number::from(if matches!(&ctx.virus_status, VirusStatus::Unknown) {
                    0.0
                } else {
                    1.1
                }),
                &value.to_number(),
            ),
            MatchType::List => false,
        };

        if !captured_values.is_empty() {
            ctx.set_match_variables(captured_values);
        }

        TestResult::Bool(result ^ self.is_not)
    }
}

impl SpamStatus {
    pub fn from_number(number: u32) -> Self {
        match number {
            1 => SpamStatus::Ham,
            2..=9 => SpamStatus::MaybeSpam(number as f64 / 10.0),
            10 => SpamStatus::Spam,
            _ => SpamStatus::Unknown,
        }
    }

    pub(crate) fn as_number(&self) -> Variable {
        Variable::Integer(match self {
            SpamStatus::Unknown => 0,
            SpamStatus::Ham => 1,
            SpamStatus::MaybeSpam(pct) => {
                let n = (pct * 10.0) as i64;
                if n < 2 {
                    2
                } else if n > 9 {
                    9
                } else {
                    n
                }
            }
            SpamStatus::Spam => 10,
        })
    }

    pub(crate) fn as_percentage(&self) -> Variable {
        Variable::Integer(match self {
            SpamStatus::Unknown | SpamStatus::Ham => 0,
            SpamStatus::MaybeSpam(pct) => {
                let n = (pct * 100.0).ceil() as i64;
                if n > 100 {
                    100
                } else if n < 1 {
                    1
                } else {
                    n
                }
            }
            SpamStatus::Spam => 100,
        })
    }
}

impl VirusStatus {
    pub fn from_number(number: u32) -> Self {
        match number {
            1 => VirusStatus::Clean,
            2 => VirusStatus::Replaced,
            3 => VirusStatus::Cured,
            4 => VirusStatus::MaybeVirus,
            5 => VirusStatus::Virus,
            _ => VirusStatus::Unknown,
        }
    }

    pub(crate) fn as_number(&self) -> Variable {
        Variable::Integer(match self {
            VirusStatus::Unknown => 0,
            VirusStatus::Clean => 1,
            VirusStatus::Replaced => 2,
            VirusStatus::Cured => 3,
            VirusStatus::MaybeVirus => 4,
            VirusStatus::Virus => 5,
        })
    }
}

impl From<u32> for SpamStatus {
    fn from(number: u32) -> Self {
        SpamStatus::from_number(number)
    }
}

impl From<i32> for SpamStatus {
    fn from(number: i32) -> Self {
        SpamStatus::from_number(number as u32)
    }
}

impl From<usize> for SpamStatus {
    fn from(number: usize) -> Self {
        SpamStatus::from_number(number as u32)
    }
}

impl From<u32> for VirusStatus {
    fn from(number: u32) -> Self {
        VirusStatus::from_number(number)
    }
}

impl From<i32> for VirusStatus {
    fn from(number: i32) -> Self {
        VirusStatus::from_number(number as u32)
    }
}

impl From<usize> for VirusStatus {
    fn from(number: usize) -> Self {
        VirusStatus::from_number(number as u32)
    }
}
