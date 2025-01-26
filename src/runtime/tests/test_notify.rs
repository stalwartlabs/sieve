/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::borrow::Cow;

use crate::{
    compiler::{
        grammar::{
            tests::test_notify::{TestNotifyMethodCapability, TestValidNotifyMethod},
            MatchType,
        },
        Number,
    },
    runtime::actions::action_notify::validate_uri,
    Context,
};

use super::TestResult;

impl TestValidNotifyMethod {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut num_valid = 0;

        for uri in &self.notification_uris {
            let uri_ = ctx.eval_value(uri);
            let uri = uri_.to_string();
            if let Some(scheme) = validate_uri(uri.as_ref()) {
                if ctx
                    .runtime
                    .valid_notification_uris
                    .contains(&Cow::from(scheme))
                    || ctx.runtime.valid_notification_uris.contains(&uri)
                {
                    num_valid += 1;
                }
            }
        }

        TestResult::Bool((num_valid == self.notification_uris.len()) ^ self.is_not)
    }
}

impl TestNotifyMethodCapability {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let uri_ = ctx.eval_value(&self.notification_uri);
        let uri = uri_.to_string();
        if !ctx
            .eval_value(&self.notification_capability)
            .to_string()
            .eq_ignore_ascii_case("online")
            || !validate_uri(uri.as_ref()).is_some_and( |scheme| {
                ctx.runtime
                    .valid_notification_uris
                    .contains(&Cow::from(scheme))
                    || ctx.runtime.valid_notification_uris.contains(&uri)
            })
        {
            return TestResult::Bool(false ^ self.is_not);
        }

        if let MatchType::Count(rel_match) = &self.match_type {
            for key in &self.key_list {
                if rel_match.cmp(&Number::from(1.0), &ctx.eval_value(key).to_number()) {
                    return TestResult::Bool(true ^ self.is_not);
                }
            }
        } else {
            for pattern in &self.key_list {
                let key = ctx.eval_value(pattern);
                if match &self.match_type {
                    MatchType::Is => self.comparator.is(&"maybe", &key),
                    MatchType::Contains => {
                        self.comparator.contains("maybe", key.to_string().as_ref())
                    }
                    MatchType::Value(relation) => {
                        self.comparator.relational(relation, &"maybe", &key)
                    }
                    MatchType::Matches(_) => self.comparator.matches(
                        "maybe",
                        key.to_string().as_ref(),
                        0,
                        &mut Vec::new(),
                    ),
                    MatchType::Regex(_) => {
                        self.comparator
                            .regex(pattern, &key, "maybe", 0, &mut Vec::new())
                    }
                    _ => false,
                } {
                    return TestResult::Bool(true ^ self.is_not);
                }
            }
        }

        TestResult::Bool(false ^ self.is_not)
    }
}
