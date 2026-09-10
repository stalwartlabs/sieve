/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{
    Context, Metadata, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::RuntimeError,
};

impl<'x> Context<'x> {
    pub(crate) fn test_metadata(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestMetadata,
    ) -> Result<TestResult, RuntimeError> {
        let mailbox = if test.metadata.kind == 1 {
            Some(self.eval_value(script, test.metadata.name)?.into_string())
        } else {
            None
        };
        let annotation = self
            .eval_value(script, test.metadata.annotation)?
            .into_string();

        let Some((_, value)) = [&self.metadata, &self.runtime.metadata]
            .into_iter()
            .flatten()
            .find(|(m, _)| match (m, &mailbox) {
                (Metadata::Server { annotation: a }, None) => a.eq_ignore_ascii_case(&annotation),
                (
                    Metadata::Mailbox {
                        name: a,
                        annotation: c,
                    },
                    Some(b),
                ) => a.eq(b) && c.eq_ignore_ascii_case(&annotation),
                _ => false,
            })
        else {
            return Ok(TestResult::Bool(false ^ test.is_not));
        };
        let value = value.as_ref();

        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();
        let mut result = false;

        if let MatchType::Count(rel_match) = &match_type {
            result = self
                .eval_values(script, test.key_list)?
                .iter()
                .any(|key| rel_match.cmp(&Number::Float(1.0), &key.to_number()));
        } else {
            let keys = self.eval_keys(script, test.key_list)?;
            let mut captured_values = Vec::new();

            for key in &keys {
                if self.key_matches(
                    script,
                    &comparator,
                    &match_type,
                    key,
                    value,
                    &mut captured_values,
                )? {
                    result = true;
                    break;
                }
            }

            if !captured_values.is_empty() {
                self.set_match_variables(captured_values);
            }
        }

        Ok(TestResult::Bool(result ^ test.is_not))
    }

    pub(crate) fn test_metadata_exists(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestMetadataExists,
    ) -> Result<TestResult, RuntimeError> {
        let mailbox = self
            .eval_opt(script, test.mailbox)?
            .map(|mailbox| mailbox.into_string());
        let mut annotations = self.eval_values(script, test.annotation_names)?;

        for (metadata, _) in [&self.metadata, &self.runtime.metadata]
            .into_iter()
            .flatten()
        {
            match (metadata, mailbox.as_deref()) {
                (Metadata::Server { annotation }, None) => {
                    annotations.retain(|a| !a.to_string().eq_ignore_ascii_case(annotation))
                }
                (Metadata::Mailbox { name, annotation }, Some(mailbox)) if name.eq(mailbox) => {
                    annotations.retain(|a| !a.to_string().eq_ignore_ascii_case(annotation));
                }
                _ => (),
            }
            if annotations.is_empty() {
                return Ok(TestResult::Bool(true ^ test.is_not));
            }
        }

        Ok(TestResult::Bool(false ^ test.is_not))
    }
}
