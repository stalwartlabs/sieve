use mail_parser::{
    decoders::html::html_to_text, Message, MessageAttachment, MimeHeaders, PartType,
};

use crate::{
    compiler::grammar::{
        tests::test_body::{BodyTransform, TestBody},
        MatchType,
    },
    Context,
};

use super::mime::{ContentTypeFilter, NestedParts};

pub(crate) const MAX_NEST_LEVELS: usize = 3;

impl TestBody {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) -> bool {
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
                        return false ^ self.is_not;
                    }
                }
                ct_filter
            }
        };

        let result = if let MatchType::Count(rel_match) = &self.match_type {
            let mut count = 0;
            let mut result = false;

            message.find_nested_parts(
                ctx,
                &ct_filter,
                MAX_NEST_LEVELS,
                &mut |_part, _raw_message| {
                    count += 1;
                    false
                },
            );

            for key in &self.key_list {
                if rel_match.cmp_num(count as f64, ctx.eval_string(key).as_ref()) {
                    result = true;
                    break;
                }
            }

            result
        } else {
            message.find_nested_parts(
                ctx,
                &ct_filter,
                MAX_NEST_LEVELS,
                &mut |part, raw_message| {
                    let text = match (&self.body_transform, &part.body) {
                        (BodyTransform::Content(_), PartType::Message(message)) => match message {
                            MessageAttachment::Parsed(message) => {
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
                            MessageAttachment::Raw(raw_message) => {
                                let mut lf_last = false;
                                let mut hdr_pos = 0;
                                for (pos, &ch) in raw_message.iter().enumerate() {
                                    match ch {
                                        b'\n' => {
                                            if !lf_last {
                                                lf_last = true;
                                            } else {
                                                hdr_pos = pos;
                                                break;
                                            }
                                        }
                                        b'\r' => (),
                                        _ => {
                                            lf_last = false;
                                        }
                                    }
                                }
                                if hdr_pos == 0 {
                                    return false;
                                }
                                String::from_utf8_lossy(&raw_message[..hdr_pos])
                            }
                        },
                        (BodyTransform::Content(_), PartType::Multipart(_)) => {
                            if let Some(boundary) = part
                                .get_content_type()
                                .and_then(|ct| ct.get_attribute("boundary"))
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
                            if part.raw_end_offset() > part.raw_body_offset() {
                                String::from_utf8_lossy(
                                    raw_message
                                        .get(part.raw_body_offset()..part.raw_end_offset())
                                        .unwrap_or(b""),
                                )
                            } else {
                                return false;
                            }
                        }
                        (_, PartType::Text(text))
                        | (BodyTransform::Content(_), PartType::Html(text)) => text.as_ref().into(),
                        (_, PartType::Html(html)) => html_to_text(html.as_ref()).into(),
                        (
                            BodyTransform::Text,
                            PartType::Binary(bytes) | PartType::InlineBinary(bytes),
                        ) if part.get_content_type().map_or(false, |ct| {
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
                            MatchType::Matches(_) => self.comparator.matches(
                                text.as_ref(),
                                key.as_ref(),
                                0,
                                &mut Vec::new(),
                            ),
                            MatchType::Regex(_) => self.comparator.regex(
                                text.as_ref(),
                                key.as_ref(),
                                0,
                                &mut Vec::new(),
                            ),
                            _ => false,
                        };

                        if result {
                            break;
                        }
                    }

                    result
                },
            )
        };

        result ^ self.is_not
    }
}
