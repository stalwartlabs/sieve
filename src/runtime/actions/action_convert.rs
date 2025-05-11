/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::{
    decoders::html::{html_to_text, text_to_html},
    Encoding, Header, HeaderName, HeaderValue, MimeHeaders, PartType,
};

use crate::{
    compiler::grammar::actions::action_convert::Convert, runtime::tests::TestResult, Context,
};

#[derive(Clone, Copy)]
enum Conversion {
    TextToHtml,
    TextPlainToHtml,
    HtmlToText,
}

impl Convert {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let _from_media_type = ctx.eval_value(&self.from_media_type);
        let _to_media_type = ctx.eval_value(&self.to_media_type);

        let from_media_type = _from_media_type.to_string();
        let to_media_type = _to_media_type.to_string();

        if from_media_type.eq_ignore_ascii_case(to_media_type.as_ref()) {
            return TestResult::Bool(false ^ self.is_not);
        }

        let conversion = if (from_media_type.eq_ignore_ascii_case("text")
            || from_media_type.starts_with("text/"))
            && to_media_type.eq_ignore_ascii_case("text/html")
        {
            if from_media_type.eq_ignore_ascii_case("text") {
                Conversion::TextPlainToHtml
            } else {
                Conversion::TextToHtml
            }
        } else if from_media_type.eq_ignore_ascii_case("text/html")
            && to_media_type.eq_ignore_ascii_case("text/plain")
        {
            Conversion::HtmlToText
        } else {
            return TestResult::Bool(false ^ self.is_not);
        };
        let mut did_convert = false;
        for part in ctx.message.parts.iter_mut() {
            let (new_body, ct) = match (&part.body, conversion) {
                (PartType::Html(html), Conversion::HtmlToText) => (
                    PartType::Text(html_to_text(html.as_ref()).into()),
                    "text/plain; charset=utf8",
                ),
                (PartType::Text(text), Conversion::TextToHtml) => (
                    PartType::Html(text_to_html(text.as_ref()).into()),
                    "text/html; charset=utf8",
                ),
                (PartType::Text(text), Conversion::TextPlainToHtml)
                    if part
                        .content_type()
                        .and_then(|ct| ct.c_subtype.as_ref())
                        .is_some_and(|st| st.eq_ignore_ascii_case("plain")) =>
                {
                    (
                        PartType::Html(text_to_html(text.as_ref()).into()),
                        "text/html; charset=utf8",
                    )
                }
                _ => {
                    continue;
                }
            };
            part.headers = vec![Header {
                name: HeaderName::Other("Content-Type".into()),
                value: HeaderValue::Text(ct.to_string().into()),
                offset_start: 0,
                offset_end: 0,
                offset_field: 0,
            }];
            ctx.message_size = ctx.message_size + ct.len() + new_body.len() + 16
                - (if part.offset_body != 0 {
                    (part.offset_end - part.offset_header) as usize
                } else {
                    part.body.len()
                });
            part.offset_body = 0;
            part.body = new_body;
            part.encoding = Encoding::QuotedPrintable; //Used as non-mime flag
            did_convert = true;
        }

        if did_convert {
            ctx.has_changes = true;
        }

        TestResult::Bool(did_convert ^ self.is_not)
    }
}
