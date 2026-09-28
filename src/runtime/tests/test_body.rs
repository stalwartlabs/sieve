/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{TestResult, matching::Key, mime::ContentTypeFilter};
use crate::runtime::message::{body::TextForm, headers::HeaderRef};
use crate::{
    Context, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::RuntimeError,
};
use mail_parser::MessagePart;
use mail_parser::{HeaderName, PartKind};
use smallvec::SmallVec;
use std::borrow::Cow;

const TRANSFORM_RAW: u8 = 0;
const TRANSFORM_CONTENT: u8 = 1;
const TRANSFORM_TEXT: u8 = 2;

impl<'x> Context<'x> {
    pub(crate) fn test_body(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestBody,
    ) -> Result<TestResult, RuntimeError> {
        let key_list = self.eval_keys(script, test.key_list)?;
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();
        let transform = test.body_transform.kind;

        if test.include_subject && !matches!(&match_type, MatchType::Count(_) | MatchType::List) {
            let subject = if transform != TRANSFORM_RAW {
                self.root_text(&HeaderName::Subject).unwrap_or_default()
            } else {
                self.root_part()
                    .and_then(|part| self.last_header(part, &HeaderName::Subject))
                    .map(|header| match header {
                        HeaderRef::Added(added) => added.value,
                        header => header
                            .raw_value()
                            .and_then(|raw| std::str::from_utf8(raw).ok())
                            .unwrap_or_default(),
                    })
                    .unwrap_or_default()
            };

            for key in &key_list {
                if self.body_key_matches(script, &comparator, &match_type, key, subject)? {
                    return Ok(TestResult::Bool(!test.is_not));
                }
            }
        }

        let mut ct_filter: SmallVec<[ContentTypeFilter<'x>; 4]> = SmallVec::new();
        if transform == TRANSFORM_CONTENT {
            for ct in self.eval_strings(script, test.body_transform.content_types)? {
                if ct.is_empty() {
                    break;
                } else if let Some(ctf) = ContentTypeFilter::parse(ct) {
                    ct_filter.push(ctf);
                } else {
                    return Ok(TestResult::Bool(test.is_not));
                }
            }
        }

        let result = if let MatchType::Count(rel_match) = &match_type {
            let mut count: i64 = 0;
            self.find_nested_parts(&ct_filter, &mut |_| {
                count += 1;
                false
            });

            key_list
                .iter()
                .any(|key| rel_match.cmp(&Number::from(count), &key.value.to_number()))
        } else {
            let mut error = None;
            let result = self.find_nested_parts(&ct_filter, &mut |part| {
                let Some(text) = self.body_text(part, transform) else {
                    return false;
                };

                for key in &key_list {
                    match self.body_key_matches(script, &comparator, &match_type, key, &text) {
                        Ok(true) => return true,
                        Ok(false) => (),
                        Err(err) => {
                            error = Some(err);
                            return true;
                        }
                    }
                }

                false
            });
            if let Some(err) = error {
                return Err(err);
            }
            result
        };

        Ok(TestResult::Bool(result ^ test.is_not))
    }

    fn body_text(&self, part: MessagePart<'x>, transform: u8) -> Option<Cow<'x, str>> {
        let text = match (transform, self.part_kind(part)) {
            (TRANSFORM_CONTENT, PartKind::Message(nested)) => {
                return Some(String::from_utf8_lossy(nested.root_part().raw_headers()));
            }
            (TRANSFORM_CONTENT, PartKind::Multipart) => {
                return Some(match part.boundary() {
                    Some(boundary) => multipart_text(part.raw_body(), boundary).into(),
                    None => String::from_utf8_lossy(part.raw_body()),
                });
            }
            (TRANSFORM_RAW, _) if self.edits.body(part.id()).is_some() => {
                self.part_text(part, TextForm::Raw)
            }
            (TRANSFORM_RAW, _) => self
                .part_text(part, TextForm::Raw)
                .filter(|raw| !raw.is_empty()),
            (_, PartKind::Text) | (TRANSFORM_CONTENT, PartKind::Html) => {
                self.part_text(part, TextForm::Source)
            }
            (_, PartKind::Html) => self.part_text(part, TextForm::Plain),
            (TRANSFORM_TEXT, PartKind::Binary | PartKind::InlineBinary)
                if self.part_content_type(part).is_some_and(|(ct, st)| {
                    ct.eq_ignore_ascii_case("application") && st.contains("xml")
                }) =>
            {
                self.part_text(part, TextForm::Markup)
            }
            (TRANSFORM_CONTENT, PartKind::Binary | PartKind::InlineBinary) => {
                self.part_text(part, TextForm::Decoded)
            }
            _ => None,
        };
        text.map(Cow::Borrowed)
    }

    fn body_key_matches(
        &self,
        script: &'x Sieve<'x>,
        comparator: &Comparator,
        match_type: &MatchType,
        key: &Key<'x>,
        text: &str,
    ) -> Result<bool, RuntimeError> {
        Ok(match match_type {
            MatchType::Is => comparator.is(&text, &key.value),
            MatchType::Contains => comparator.contains(text, key.value.to_string().as_ref()),
            MatchType::Value(rel_match) => comparator.relational(rel_match, &text, &key.value),
            MatchType::Matches(_) => self.glob_matches(
                script,
                comparator.is_casemap(),
                key,
                text,
                0,
                &mut Vec::new(),
            )?,
            MatchType::Regex(_) => self.regex_matches(script, key, text, 0, &mut Vec::new())?,
            MatchType::Count(_) | MatchType::List => false,
        })
    }
}

fn multipart_text(raw_body: &[u8], boundary: &str) -> String {
    let mime_body = std::str::from_utf8(raw_body).unwrap_or_default();
    let mut mime_part = String::with_capacity(64);
    if let Some((prologue, epilogue)) = mime_body.split_once(&format!("\n--{boundary}")) {
        mime_part.push_str(prologue);
        if let Some((_, epilogue)) = epilogue.rsplit_once(&format!("\n--{boundary}--")) {
            mime_part.push_str(epilogue);
        }
    }
    mime_part
}
