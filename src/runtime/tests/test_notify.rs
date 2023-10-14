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
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
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
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let uri_ = ctx.eval_value(&self.notification_uri);
        let uri = uri_.to_string();
        if !ctx
            .eval_value(&self.notification_capability)
            .to_string()
            .eq_ignore_ascii_case("online")
            || !validate_uri(uri.as_ref()).map_or(false, |scheme| {
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
