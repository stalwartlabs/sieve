/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::edits::{BodyEdit, BodyKind};
use crate::Context;
use mail_parser::{MessagePart, PartId, PartKind, html_to_text, text_to_html};
use std::borrow::Cow;

pub(crate) const TEXT_FORMS: usize = 6;

pub(crate) type BodyCache<'x> = Vec<[Option<&'x str>; TEXT_FORMS]>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub(crate) enum TextForm {
    Source = 0,
    Plain = 1,
    Html = 2,
    Decoded = 3,
    Raw = 4,
    Markup = 5,
}

impl<'x> Context<'x> {
    #[inline]
    pub(crate) fn part_kind(&self, part: MessagePart<'x>) -> PartKind<'x> {
        match self.edits.body(part.id()) {
            Some(body) => body.kind.into(),
            None => part.kind(),
        }
    }

    pub(crate) fn part_text(&self, part: MessagePart<'x>, form: TextForm) -> Option<&'x str> {
        let index = part.id() as usize;
        if let Some(text) = self
            .body_cache
            .borrow()
            .get(index)
            .and_then(|forms| forms.get(form as usize).copied().flatten())
        {
            return Some(text);
        }
        let text = match self.edits.body(part.id()) {
            Some(body) => self.edited_text(*body, form)?,
            None => self.decode_text(part, form)?,
        };
        let mut cache = self.body_cache.borrow_mut();
        if cache.len() <= index {
            cache.resize(
                self.message.parts().len().max(index + 1),
                [None; TEXT_FORMS],
            );
        }
        if let Some(slot) = cache
            .get_mut(index)
            .and_then(|forms| forms.get_mut(form as usize))
        {
            *slot = Some(text);
        }
        Some(text)
    }

    fn decode_text(&self, part: MessagePart<'x>, form: TextForm) -> Option<&'x str> {
        match form {
            TextForm::Source => part.text().map(|text| self.keep_text(text)),
            TextForm::Plain => match part.kind() {
                PartKind::Text => self.part_text(part, TextForm::Source),
                PartKind::Html => self
                    .part_text(part, TextForm::Source)
                    .map(|html| self.keep_text(html_to_text(html).into())),
                _ => None,
            },
            TextForm::Html => match part.kind() {
                PartKind::Html => self.part_text(part, TextForm::Source),
                PartKind::Text => self
                    .part_text(part, TextForm::Source)
                    .map(|text| self.keep_text(text_to_html(text).into())),
                _ => None,
            },
            TextForm::Decoded => Some(self.keep_bytes(part.decoded())),
            TextForm::Raw => Some(self.keep_bytes(Cow::Borrowed(part.raw_body()))),
            TextForm::Markup => self
                .part_text(part, TextForm::Decoded)
                .map(|markup| self.keep_text(html_to_text(markup).into())),
        }
    }

    fn edited_text(&self, body: BodyEdit<'x>, form: TextForm) -> Option<&'x str> {
        match (form, body.kind) {
            (TextForm::Plain, BodyKind::Html) => Some(self.keep_charged(html_to_text(body.text))),
            (TextForm::Html, BodyKind::Text) => Some(self.keep_charged(text_to_html(body.text))),
            (TextForm::Markup, _) => None,
            _ => Some(body.text),
        }
    }

    #[inline]
    pub(crate) fn keep_text(&self, text: Cow<'x, str>) -> &'x str {
        match text {
            Cow::Borrowed(text) => text,
            Cow::Owned(text) => self.arena.keep_text(text),
        }
    }

    fn keep_charged(&self, text: String) -> &'x str {
        self.arena.keep_charged_text(text).unwrap_or_else(|| {
            self.note_oom();
            ""
        })
    }

    fn keep_bytes(&self, bytes: Cow<'x, [u8]>) -> &'x str {
        match bytes {
            Cow::Borrowed(bytes) => self.keep_text(String::from_utf8_lossy(bytes)),
            Cow::Owned(bytes) => self.arena.keep_text(
                String::from_utf8(bytes)
                    .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned()),
            ),
        }
    }

    pub(crate) fn set_part_body(&mut self, part: PartId, body: BodyEdit<'x>) {
        self.edits.set_body(part, body);
        if let Some(forms) = self.body_cache.get_mut().get_mut(part as usize) {
            *forms = [None; TEXT_FORMS];
        }
    }

    pub(crate) fn reset_body_cache(&mut self) {
        self.body_cache.get_mut().clear();
    }
}

impl From<BodyKind> for PartKind<'_> {
    fn from(kind: BodyKind) -> Self {
        match kind {
            BodyKind::Text => PartKind::Text,
            BodyKind::Html => PartKind::Html,
        }
    }
}
