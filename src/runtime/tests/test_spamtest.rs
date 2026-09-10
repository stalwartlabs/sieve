/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{
    TestResult,
    matching::{Key, Pattern},
};
use crate::{
    Context, Sieve, SpamStatus, VirusStatus,
    bytecode::{
        ops,
        rec::{Rec, tag},
    },
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::{RuntimeError, Variable},
};

impl<'x> Context<'x> {
    pub(crate) fn test_spamtest(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestSpamTest,
    ) -> Result<TestResult, RuntimeError> {
        let status = if test.percent {
            self.spam_status.as_percentage()
        } else {
            self.spam_status.as_number()
        };
        let count = if matches!(self.spam_status, SpamStatus::Unknown) {
            0.0
        } else {
            1.1
        };
        self.test_status(script, test, status, count)
    }

    pub(crate) fn test_virustest(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestVirusTest,
    ) -> Result<TestResult, RuntimeError> {
        let status = self.virus_status.as_number();
        let count = if matches!(self.virus_status, VirusStatus::Unknown) {
            0.0
        } else {
            1.1
        };
        let test = ops::TestSpamTest {
            value: test.value,
            match_type: test.match_type,
            comparator: test.comparator,
            percent: false,
            is_not: test.is_not,
        };
        self.test_status(script, &test, status, count)
    }

    fn test_status(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestSpamTest,
        status: Variable<'static>,
        count: f64,
    ) -> Result<TestResult, RuntimeError> {
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();
        let key = self.eval_key(script, test.value)?;
        let mut captured_values = Vec::new();

        let result = match &match_type {
            MatchType::Count(rel_match) => {
                rel_match.cmp(&Number::from(count), &key.value.to_number())
            }
            MatchType::List => false,
            _ => self.key_matches(
                script,
                &comparator,
                &match_type,
                &key,
                status.to_string().as_ref(),
                &mut captured_values,
            )?,
        };

        if !captured_values.is_empty() {
            self.set_match_variables(captured_values);
        }

        Ok(TestResult::Bool(result ^ test.is_not))
    }

    fn eval_key(&self, script: &'x Sieve<'x>, rec: Rec) -> Result<Key<'x>, RuntimeError> {
        let pattern = match rec.tag {
            tag::GLOB => Pattern::Glob(rec.c),
            tag::REGEX => Pattern::Regex(rec.c),
            _ => Pattern::Dynamic,
        };
        Ok(Key {
            value: self.eval_value(script, rec)?,
            pattern,
        })
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

    pub(crate) fn as_number(&self) -> Variable<'static> {
        Variable::Integer(match self {
            SpamStatus::Unknown => 0,
            SpamStatus::Ham => 1,
            SpamStatus::MaybeSpam(pct) => ((pct * 10.0) as i64).clamp(2, 9),
            SpamStatus::Spam => 10,
        })
    }

    pub(crate) fn as_percentage(&self) -> Variable<'static> {
        Variable::Integer(match self {
            SpamStatus::Unknown | SpamStatus::Ham => 0,
            SpamStatus::MaybeSpam(pct) => ((pct * 100.0).ceil() as i64).clamp(1, 100),
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

    pub(crate) fn as_number(&self) -> Variable<'static> {
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
