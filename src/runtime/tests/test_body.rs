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

use mail_parser::{decoders::html::html_to_text, MimeHeaders, PartType};

use crate::{
    compiler::{
        grammar::{
            tests::test_body::{BodyTransform, TestBody},
            MatchType,
        },
        Number,
    },
    runtime::Variable,
    Context,
};

use super::{mime::ContentTypeFilter, TestResult};

impl TestBody {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        // Check Subject (not a Sieve standard)
        let key_list = ctx.eval_values(&self.key_list);
        if self.include_subject {
            let subject = if !matches!(&self.body_transform, BodyTransform::Raw) {
                ctx.message.subject().unwrap_or_default()
            } else {
                ctx.message.header_raw("Subject").unwrap_or_default()
            };

            for (key, pattern) in key_list.iter().zip(self.key_list.iter()) {
                let result = match &self.match_type {
                    MatchType::Is => self.comparator.is(&Variable::from(subject), key),
                    MatchType::Contains => self.comparator.contains(subject, key.to_cow().as_ref()),
                    MatchType::Value(rel_match) => {
                        self.comparator
                            .relational(rel_match, &Variable::from(subject), key)
                    }
                    MatchType::Matches(_) => {
                        self.comparator
                            .matches(subject, key.to_cow().as_ref(), 0, &mut Vec::new())
                    }
                    MatchType::Regex(_) => {
                        self.comparator
                            .regex(pattern, key, subject, 0, &mut Vec::new())
                    }
                    _ => break,
                };

                if result {
                    return TestResult::Bool(result ^ self.is_not);
                }
            }
        }

        let ct_filter = match &self.body_transform {
            BodyTransform::Text | BodyTransform::Raw => Vec::new(),
            BodyTransform::Content(values) => {
                let mut ct_filter = Vec::with_capacity(values.len());
                for ct in values {
                    let ct = ctx.eval_value(ct);
                    if ct.is_empty() {
                        break;
                    } else if let Some(ctf) = ContentTypeFilter::parse(ct.into_cow().as_ref()) {
                        ct_filter.push(ctf);
                    } else {
                        return TestResult::Bool(false ^ self.is_not);
                    }
                }
                ct_filter
            }
        };

        let result = if let MatchType::Count(rel_match) = &self.match_type {
            let mut count = 0;
            let mut result = false;

            ctx.find_nested_parts(&ctx.message, &ct_filter, &mut |_part, _raw_message| {
                count += 1;
                false
            });

            for key in &self.key_list {
                if rel_match.cmp(&Number::from(count), &ctx.eval_value(key).to_number()) {
                    result = true;
                    break;
                }
            }

            result
        } else {
            ctx.find_nested_parts(&ctx.message, &ct_filter, &mut |part, raw_message| {
                let text = match (&self.body_transform, &part.body) {
                    (BodyTransform::Content(_), PartType::Message(message)) => {
                        if let Some(part) = message.parts.get(0) {
                            String::from_utf8_lossy(
                                raw_message
                                    .get(part.raw_header_offset()..part.raw_body_offset())
                                    .unwrap_or(b""),
                            )
                        } else {
                            return false;
                        }
                    }
                    (BodyTransform::Content(_), PartType::Multipart(_)) => {
                        if let Some(boundary) =
                            part.content_type().and_then(|ct| ct.attribute("boundary"))
                        {
                            let mime_body = std::str::from_utf8(
                                raw_message
                                    .get(part.raw_body_offset()..part.raw_end_offset())
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
                                    .get(part.raw_body_offset()..part.raw_end_offset())
                                    .unwrap_or(b""),
                            )
                        }
                    }
                    (BodyTransform::Raw, _) => {
                        match &part.body {
                            PartType::Text(text) if part.raw_body_offset() == 0 => {
                                // Inserted part
                                text.as_ref().into()
                            }
                            _ if part.raw_end_offset() > part.raw_body_offset() => {
                                String::from_utf8_lossy(
                                    raw_message
                                        .get(part.raw_body_offset()..part.raw_end_offset())
                                        .unwrap_or(b""),
                                )
                            }
                            _ => return false,
                        }
                    }
                    (_, PartType::Text(text))
                    | (BodyTransform::Content(_), PartType::Html(text)) => text.as_ref().into(),
                    (_, PartType::Html(html)) => html_to_text(html.as_ref()).into(),
                    (
                        BodyTransform::Text,
                        PartType::Binary(bytes) | PartType::InlineBinary(bytes),
                    ) if part.content_type().map_or(false, |ct| {
                        ct.c_type.eq_ignore_ascii_case("application")
                            && ct.c_subtype.as_ref().map_or(false, |st| st.contains("xml"))
                    }) =>
                    {
                        html_to_text(std::str::from_utf8(bytes.as_ref()).unwrap_or("")).into()
                    }
                    (
                        BodyTransform::Content(_),
                        PartType::Binary(bytes) | PartType::InlineBinary(bytes),
                    ) => String::from_utf8_lossy(bytes.as_ref()),
                    _ => {
                        return false;
                    }
                };
                let mut result = false;

                for (key, pattern) in key_list.iter().zip(self.key_list.iter()) {
                    result = match &self.match_type {
                        MatchType::Is => self.comparator.is(&Variable::from(text.as_ref()), key),
                        MatchType::Contains => self
                            .comparator
                            .contains(text.as_ref(), key.to_cow().as_ref()),
                        MatchType::Value(rel_match) => self.comparator.relational(
                            rel_match,
                            &Variable::from(text.as_ref()),
                            key,
                        ),
                        MatchType::Matches(_) => self.comparator.matches(
                            text.as_ref(),
                            key.to_cow().as_ref(),
                            0,
                            &mut Vec::new(),
                        ),
                        MatchType::Regex(_) => {
                            self.comparator
                                .regex(pattern, key, text.as_ref(), 0, &mut Vec::new())
                        }
                        _ => false,
                    };

                    if result {
                        break;
                    }
                }

                result
            })
        };

        TestResult::Bool(result ^ self.is_not)
    }
}
