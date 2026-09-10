/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{TestResult, matching::Key, mime::ContentTypeFilter};
use crate::{
    Context, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::RuntimeError,
};
use mail_parser::{MimeHeaders, PartType, decoders::html::html_to_text};
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
                self.message.subject().unwrap_or_default()
            } else {
                self.message.header_raw("Subject").unwrap_or_default()
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
            self.find_nested_parts(&self.message, &ct_filter, &mut |_part, _raw_message| {
                count += 1;
                false
            });

            key_list
                .iter()
                .any(|key| rel_match.cmp(&Number::from(count), &key.value.to_number()))
        } else {
            let mut error = None;
            let result =
                self.find_nested_parts(&self.message, &ct_filter, &mut |part, raw_message| {
                    let text: Cow<str> = match (transform, &part.body) {
                        (TRANSFORM_CONTENT, PartType::Message(message)) => {
                            if let Some(part) = message.parts.first() {
                                String::from_utf8_lossy(
                                    raw_message
                                        .get(
                                            part.raw_header_offset() as usize
                                                ..part.raw_body_offset() as usize,
                                        )
                                        .unwrap_or(b""),
                                )
                            } else {
                                return false;
                            }
                        }
                        (TRANSFORM_CONTENT, PartType::Multipart(_)) => {
                            if let Some(boundary) =
                                part.content_type().and_then(|ct| ct.attribute("boundary"))
                            {
                                let mime_body = std::str::from_utf8(
                                    raw_message
                                        .get(
                                            part.raw_body_offset() as usize
                                                ..part.raw_end_offset() as usize,
                                        )
                                        .unwrap_or(b""),
                                )
                                .unwrap_or("");
                                let mut mime_part = String::with_capacity(64);
                                if let Some((prologue, epilogue)) =
                                    mime_body.split_once(&format!("\n--{boundary}"))
                                {
                                    mime_part.push_str(prologue);
                                    if let Some((_, epilogue)) =
                                        epilogue.rsplit_once(&format!("\n--{boundary}--"))
                                    {
                                        mime_part.push_str(epilogue);
                                    }
                                }
                                mime_part.into()
                            } else {
                                String::from_utf8_lossy(
                                    raw_message
                                        .get(
                                            part.raw_body_offset() as usize
                                                ..part.raw_end_offset() as usize,
                                        )
                                        .unwrap_or(b""),
                                )
                            }
                        }
                        (TRANSFORM_RAW, _) => match &part.body {
                            PartType::Text(text) if part.raw_body_offset() == 0 => {
                                text.as_ref().into()
                            }
                            _ if part.raw_end_offset() > part.raw_body_offset() => {
                                String::from_utf8_lossy(
                                    raw_message
                                        .get(
                                            part.raw_body_offset() as usize
                                                ..part.raw_end_offset() as usize,
                                        )
                                        .unwrap_or(b""),
                                )
                            }
                            _ => return false,
                        },
                        (_, PartType::Text(text)) | (TRANSFORM_CONTENT, PartType::Html(text)) => {
                            text.as_ref().into()
                        }
                        (_, PartType::Html(html)) => html_to_text(html.as_ref()).into(),
                        (
                            TRANSFORM_TEXT,
                            PartType::Binary(bytes) | PartType::InlineBinary(bytes),
                        ) if part.content_type().is_some_and(|ct| {
                            ct.c_type.eq_ignore_ascii_case("application")
                                && ct.c_subtype.as_ref().is_some_and(|st| st.contains("xml"))
                        }) =>
                        {
                            html_to_text(std::str::from_utf8(bytes.as_ref()).unwrap_or("")).into()
                        }
                        (
                            TRANSFORM_CONTENT,
                            PartType::Binary(bytes) | PartType::InlineBinary(bytes),
                        ) => String::from_utf8_lossy(bytes.as_ref()),
                        _ => {
                            return false;
                        }
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
