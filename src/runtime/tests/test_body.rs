/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
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
    compiler::grammar::{
        tests::test_body::{BodyTransform, TestBody},
        MatchType,
    },
    Context,
};

use super::{mime::ContentTypeFilter, TestResult};

impl TestBody {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let key_list = ctx.eval_strings(&self.key_list);
        let ct_filter = match &self.body_transform {
            BodyTransform::Text | BodyTransform::Raw => Vec::new(),
            BodyTransform::Content(values) => {
                let mut ct_filter = Vec::with_capacity(values.len());
                for ct in values {
                    let ct = ctx.eval_string(ct);
                    if ct.is_empty() {
                        break;
                    } else if let Some(ctf) = ContentTypeFilter::parse(ct.as_ref()) {
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
                if rel_match.cmp_num(count as f64, ctx.eval_string(key).as_ref()) {
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
                        if let Some(boundary) = part
                            .content_type()
                            .and_then(|ct| ct.attribute("boundary"))
                        {
                            let mime_body = std::str::from_utf8(
                                raw_message
                                    .get(part.raw_body_offset()..part.raw_end_offset())
                                    .unwrap_or(b""),
                            )
                            .unwrap_or("");
                            let mut mime_part = String::with_capacity(64);
                            if let Some((prologue, epilogue)) =
                                mime_body.split_once(&format!("\n--{}", boundary))
                            {
                                mime_part.push_str(prologue);
                                if let Some((_, epilogue)) =
                                    epilogue.rsplit_once(&format!("\n--{}--", boundary))
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

                for key in &key_list {
                    result = match &self.match_type {
                        MatchType::Is => self.comparator.is(text.as_ref(), key.as_ref()),
                        MatchType::Contains => {
                            self.comparator.contains(text.as_ref(), key.as_ref())
                        }
                        MatchType::Value(rel_match) => {
                            self.comparator
                                .relational(rel_match, text.as_ref(), key.as_ref())
                        }
                        MatchType::Matches(_) => {
                            self.comparator
                                .matches(text.as_ref(), key.as_ref(), 0, &mut Vec::new())
                        }
                        MatchType::Regex(_) => {
                            self.comparator
                                .regex(text.as_ref(), key.as_ref(), 0, &mut Vec::new())
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
