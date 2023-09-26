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
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let from_media_type = ctx.eval_value(&self.from_media_type).into_cow();
        let to_media_type = ctx.eval_value(&self.to_media_type).into_cow();

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
                        .map_or(false, |st| st.eq_ignore_ascii_case("plain")) =>
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
                    part.offset_end - part.offset_header
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
