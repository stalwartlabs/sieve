use std::borrow::Cow;

use crate::{
    compiler::grammar::{
        tests::test_spamtest::{TestSpamTest, TestVirusTest},
        MatchType,
    },
    Context, SpamStatus, VirusStatus,
};

use super::TestResult;

impl TestSpamTest {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let status = if self.percent {
            ctx.spam_status.as_percentage()
        } else {
            ctx.spam_status.as_number()
        };
        let value = ctx.eval_string(&self.value);
        let mut captured_values = Vec::new();

        let result = match &self.match_type {
            MatchType::Is => self.comparator.is(status.as_ref(), value.as_ref()),
            MatchType::Contains => self.comparator.contains(status.as_ref(), value.as_ref()),
            MatchType::Value(rel_match) => {
                self.comparator
                    .relational(rel_match, status.as_ref(), value.as_ref())
            }
            MatchType::Matches(capture_positions) => self.comparator.matches(
                status.as_ref(),
                value.as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Regex(capture_positions) => self.comparator.regex(
                status.as_ref(),
                value.as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Count(rel_match) => rel_match.cmp_num(
                if matches!(&ctx.spam_status, SpamStatus::Unknown) {
                    0.0
                } else {
                    1.1
                },
                value.as_ref(),
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
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let status = ctx.virus_status.as_number();
        let value = ctx.eval_string(&self.value);
        let mut captured_values = Vec::new();

        let result = match &self.match_type {
            MatchType::Is => self.comparator.is(status.as_ref(), value.as_ref()),
            MatchType::Contains => self.comparator.contains(status.as_ref(), value.as_ref()),
            MatchType::Value(rel_match) => {
                self.comparator
                    .relational(rel_match, status.as_ref(), value.as_ref())
            }
            MatchType::Matches(capture_positions) => self.comparator.regex(
                status.as_ref(),
                value.as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Regex(capture_positions) => self.comparator.regex(
                status.as_ref(),
                value.as_ref(),
                *capture_positions,
                &mut captured_values,
            ),
            MatchType::Count(rel_match) => rel_match.cmp_num(
                if matches!(&ctx.virus_status, VirusStatus::Unknown) {
                    0.0
                } else {
                    1.1
                },
                value.as_ref(),
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

    pub fn as_number(&self) -> Cow<'static, str> {
        match self {
            SpamStatus::Unknown => "0".into(),
            SpamStatus::Ham => "1".into(),
            SpamStatus::MaybeSpam(pct) => {
                let n = (pct * 10.0) as u32;
                (if n < 2 {
                    2
                } else if n > 9 {
                    9
                } else {
                    n
                })
                .to_string()
                .into()
            }
            SpamStatus::Spam => "10".into(),
        }
    }

    pub fn as_percentage(&self) -> Cow<'static, str> {
        match self {
            SpamStatus::Unknown | SpamStatus::Ham => "0".into(),
            SpamStatus::MaybeSpam(pct) => {
                let n = (pct * 100.0).ceil() as u32;
                (if n > 100 {
                    100
                } else if n < 1 {
                    1
                } else {
                    n
                })
                .to_string()
                .into()
            }
            SpamStatus::Spam => "100".into(),
        }
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

    pub fn as_number(&self) -> &'static str {
        match self {
            VirusStatus::Unknown => "0",
            VirusStatus::Clean => "1",
            VirusStatus::Replaced => "2",
            VirusStatus::Cured => "3",
            VirusStatus::MaybeVirus => "4",
            VirusStatus::Virus => "5",
        }
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
