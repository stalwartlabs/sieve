/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    compiler::{
        grammar::{
            tests::test_mailbox::{TestMetadata, TestMetadataExists},
            MatchType,
        },
        Number,
    },
    Context, Metadata,
};

use super::TestResult;

impl TestMetadata {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let metadata = match &self.medatata {
            Metadata::Server { annotation } => Metadata::Server {
                annotation: ctx.eval_value(annotation).to_string().into_owned(),
            },
            Metadata::Mailbox { name, annotation } => Metadata::Mailbox {
                name: ctx.eval_value(name).to_string().into_owned(),
                annotation: ctx.eval_value(annotation).to_string().into_owned(),
            },
        };

        let value = if let Some((_, value)) = [&ctx.metadata, &ctx.runtime.metadata]
            .into_iter()
            .flatten()
            .find(|(m, _)| match (m, &metadata) {
                (Metadata::Server { annotation: a }, Metadata::Server { annotation: b }) => {
                    a.eq_ignore_ascii_case(b)
                }
                (
                    Metadata::Mailbox {
                        name: a,
                        annotation: c,
                    },
                    Metadata::Mailbox {
                        name: b,
                        annotation: d,
                    },
                ) => a.eq(b) && c.eq_ignore_ascii_case(d),
                _ => false,
            }) {
            value.as_ref()
        } else {
            return TestResult::Bool(false ^ self.is_not);
        };

        let mut result = false;
        if let MatchType::Count(match_type) = &self.match_type {
            for key in &self.key_list {
                if match_type.cmp(&Number::Float(1.0), &ctx.eval_value(key).to_number()) {
                    result = true;
                    break;
                }
            }
        } else {
            let mut captured_values = Vec::new();

            for pattern in &self.key_list {
                let key = ctx.eval_value(pattern);
                result = match &self.match_type {
                    MatchType::Is => self.comparator.is(&value, &key),
                    MatchType::Contains => {
                        self.comparator.contains(value, key.to_string().as_ref())
                    }
                    MatchType::Value(relation) => {
                        self.comparator.relational(relation, &value, &key)
                    }
                    MatchType::Matches(capture_positions) => self.comparator.matches(
                        value,
                        key.to_string().as_ref(),
                        *capture_positions,
                        &mut captured_values,
                    ),
                    MatchType::Regex(capture_positions) => self.comparator.regex(
                        pattern,
                        &key,
                        value,
                        *capture_positions,
                        &mut captured_values,
                    ),
                    _ => false,
                };

                if result {
                    break;
                }
            }

            if !captured_values.is_empty() {
                ctx.set_match_variables(captured_values);
            }
        }

        TestResult::Bool(result ^ self.is_not)
    }
}

impl TestMetadataExists {
    pub(crate) fn exec(&self, ctx: &Context) -> TestResult {
        let mailbox = self
            .mailbox
            .as_ref()
            .map(|s| ctx.eval_value(s).to_string().into_owned());
        let mut annotations = ctx.eval_values(&self.annotation_names);

        for (metadata, _) in [&ctx.metadata, &ctx.runtime.metadata].into_iter().flatten() {
            match (metadata, mailbox.as_ref()) {
                (Metadata::Server { annotation }, None) => {
                    annotations.retain(|a| !a.to_string().eq_ignore_ascii_case(annotation))
                }
                (Metadata::Mailbox { name, annotation }, Some(mailbox)) if name.eq(mailbox) => {
                    annotations.retain(|a| !a.to_string().eq_ignore_ascii_case(annotation));
                }
                _ => (),
            }
            if annotations.is_empty() {
                return TestResult::Bool(true ^ self.is_not);
            }
        }

        TestResult::Bool(false ^ self.is_not)
    }
}
