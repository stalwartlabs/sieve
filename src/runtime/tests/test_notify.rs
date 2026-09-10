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
    runtime::{RuntimeError, actions::action_notify::validate_uri},
};

impl<'x> Context<'x> {
    pub(crate) fn test_notify_method_capability(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestNotifyMethodCapability,
    ) -> Result<TestResult, RuntimeError> {
        let uri = self.eval_value(script, test.notification_uri)?;
        let uri = uri.to_string();
        let capability = self.eval_value(script, test.notification_capability)?;
        if !capability.to_string().eq_ignore_ascii_case("online")
            || !self.is_valid_notification_uri(uri.as_ref())
        {
            return Ok(TestResult::Bool(false ^ test.is_not));
        }

        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();

        if let MatchType::Count(rel_match) = &match_type {
            let matched = self
                .eval_values(script, test.key_list)?
                .iter()
                .any(|key| rel_match.cmp(&Number::from(1.0), &key.to_number()));
            return Ok(TestResult::Bool(matched ^ test.is_not));
        }

        let mut captured_values = Vec::new();
        for key in &self.eval_keys(script, test.key_list)? {
            let matched = match &match_type {
                MatchType::Is => comparator.is(&"maybe", &key.value),
                MatchType::Contains => comparator.contains("maybe", key.value.to_string().as_ref()),
                MatchType::Value(relation) => comparator.relational(relation, &"maybe", &key.value),
                MatchType::Matches(_) => self.glob_matches(
                    script,
                    comparator.is_casemap(),
                    key,
                    "maybe",
                    0,
                    &mut captured_values,
                )?,
                MatchType::Regex(_) => {
                    self.regex_matches(script, key, "maybe", 0, &mut captured_values)?
                }
                _ => false,
            };
            if matched {
                return Ok(TestResult::Bool(true ^ test.is_not));
            }
        }

        Ok(TestResult::Bool(false ^ test.is_not))
    }

    pub(crate) fn test_valid_notify_method(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestValidNotifyMethod,
    ) -> Result<TestResult, RuntimeError> {
        let all_valid = self
            .eval_values(script, test.notification_uris)?
            .iter()
            .all(|uri| self.is_valid_notification_uri(uri.to_string().as_ref()));

        Ok(TestResult::Bool(all_valid ^ test.is_not))
    }

    fn is_valid_notification_uri(&self, uri: &str) -> bool {
        validate_uri(uri).is_some_and(|scheme| {
            self.runtime.valid_notification_uris.contains(scheme)
                || self.runtime.valid_notification_uris.contains(uri)
        })
    }
}
