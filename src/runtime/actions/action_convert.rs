/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::super::tests::TestResult;
use crate::{
    Context, Sieve,
    bytecode::ops,
    runtime::{
        RuntimeError,
        message::{
            body::TextForm,
            edits::{BodyEdit, BodyKind},
            parts::{PartCursor, Scope},
        },
    },
};
use mail_parser::{HeaderName, PartKind};

#[derive(Clone, Copy)]
enum Conversion {
    TextToHtml,
    TextPlainToHtml,
    HtmlToText,
}

impl<'x> Context<'x> {
    pub(crate) fn exec_convert(
        &mut self,
        script: &'x Sieve<'x>,
        convert: &ops::Convert,
    ) -> Result<TestResult, RuntimeError> {
        let from_media_type = self.eval_value(script, convert.from_media_type)?;
        let to_media_type = self.eval_value(script, convert.to_media_type)?;

        let from_media_type = from_media_type.to_string();
        let to_media_type = to_media_type.to_string();

        if from_media_type.eq_ignore_ascii_case(to_media_type.as_ref()) {
            return Ok(TestResult::Bool(false ^ convert.is_not));
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
            return Ok(TestResult::Bool(false ^ convert.is_not));
        };
        let mut did_convert = false;
        let mut parts = match self.root_part() {
            Some(root) => PartCursor::subtree(root, true, Scope::Message),
            None => PartCursor::default(),
        };
        while let Some(part) = self.advance(&mut parts) {
            let (text, kind, content_type) = match (self.part_kind(part), conversion) {
                (PartKind::Html, Conversion::HtmlToText) => (
                    self.part_text(part, TextForm::Plain),
                    BodyKind::Text,
                    "text/plain; charset=utf8",
                ),
                (PartKind::Text, Conversion::TextToHtml) => (
                    self.part_text(part, TextForm::Html),
                    BodyKind::Html,
                    "text/html; charset=utf8",
                ),
                (PartKind::Text, Conversion::TextPlainToHtml)
                    if self
                        .part_content_type(part)
                        .is_some_and(|(_, subtype)| subtype.eq_ignore_ascii_case("plain")) =>
                {
                    (
                        self.part_text(part, TextForm::Html),
                        BodyKind::Html,
                        "text/html; charset=utf8",
                    )
                }
                _ => continue,
            };
            let Some(text) = text else {
                continue;
            };
            let part_id = part.id();
            let previous_size = match self.edits.body(part_id) {
                Some(edited) => edited.text.len(),
                None => part.raw().len(),
            };
            self.message_size = (self.message_size + content_type.len() + text.len() + 16)
                .saturating_sub(previous_size);
            let content_type = self.add_header(HeaderName::ContentType, content_type);
            self.edits.set_headers(part_id, vec![content_type]);
            self.set_part_body(
                part_id,
                BodyEdit {
                    text,
                    kind,
                    mime: false,
                },
            );
            did_convert = true;
        }

        if did_convert {
            self.has_changes = true;
        }

        Ok(TestResult::Bool(did_convert ^ convert.is_not))
    }
}
