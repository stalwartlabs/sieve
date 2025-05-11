/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
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
                    MatchType::Is => self.comparator.is(&subject, key),
                    MatchType::Contains => {
                        self.comparator.contains(subject, key.to_string().as_ref())
                    }
                    MatchType::Value(rel_match) => {
                        self.comparator.relational(rel_match, &subject, key)
                    }
                    MatchType::Matches(_) => self.comparator.matches(
                        subject,
                        key.to_string().as_ref(),
                        0,
                        &mut Vec::new(),
                    ),
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
                    } else if let Some(ctf) = ContentTypeFilter::parse(ct.to_string().as_ref()) {
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
                    (BodyTransform::Content(_), PartType::Multipart(_)) => {
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
                    (BodyTransform::Raw, _) => {
                        match &part.body {
                            PartType::Text(text) if part.raw_body_offset() == 0 => {
                                // Inserted part
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
                        }
                    }
                    (_, PartType::Text(text))
                    | (BodyTransform::Content(_), PartType::Html(text)) => text.as_ref().into(),
                    (_, PartType::Html(html)) => html_to_text(html.as_ref()).into(),
                    (
                        BodyTransform::Text,
                        PartType::Binary(bytes) | PartType::InlineBinary(bytes),
                    ) if part.content_type().is_some_and(|ct| {
                        ct.c_type.eq_ignore_ascii_case("application")
                            && ct.c_subtype.as_ref().is_some_and(|st| st.contains("xml"))
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
                        MatchType::Is => self.comparator.is(&text.as_ref(), key),
                        MatchType::Contains => self
                            .comparator
                            .contains(text.as_ref(), key.to_string().as_ref()),
                        MatchType::Value(rel_match) => {
                            self.comparator.relational(rel_match, &text.as_ref(), key)
                        }
                        MatchType::Matches(_) => self.comparator.matches(
                            text.as_ref(),
                            key.to_string().as_ref(),
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
