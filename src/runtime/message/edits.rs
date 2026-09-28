/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::headers::HeaderRef;
use mail_parser::{MessagePart, PartId};
use std::ops::Range;

#[derive(Default)]
pub(crate) struct Edits<'x> {
    headers: Vec<(PartId, Vec<HeaderRef<'x>>)>,
    bodies: Vec<(PartId, BodyEdit<'x>)>,
    hidden: Vec<Range<PartId>>,
}

#[derive(Clone, Copy)]
pub(crate) struct BodyEdit<'x> {
    pub(crate) text: &'x str,
    pub(crate) kind: BodyKind,
    pub(crate) mime: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BodyKind {
    Text,
    Html,
}

impl<'x> Edits<'x> {
    #[inline(always)]
    pub(crate) fn headers(&self, part: PartId) -> Option<&[HeaderRef<'x>]> {
        if self.headers.is_empty() {
            return None;
        }
        self.headers
            .iter()
            .find(|(id, _)| *id == part)
            .map(|(_, headers)| headers.as_slice())
    }

    pub(crate) fn headers_mut(&mut self, part: MessagePart<'x>) -> &mut Vec<HeaderRef<'x>> {
        let id = part.id();
        let index = match self.headers.iter().position(|(edited, _)| *edited == id) {
            Some(index) => index,
            None => {
                self.headers
                    .push((id, part.headers().iter().map(HeaderRef::Parsed).collect()));
                self.headers.len() - 1
            }
        };
        &mut self.headers[index].1
    }

    pub(crate) fn set_headers(&mut self, part: PartId, headers: Vec<HeaderRef<'x>>) {
        match self.headers.iter_mut().find(|(id, _)| *id == part) {
            Some((_, edited)) => *edited = headers,
            None => self.headers.push((part, headers)),
        }
    }

    #[inline(always)]
    pub(crate) fn body(&self, part: PartId) -> Option<&BodyEdit<'x>> {
        if self.bodies.is_empty() {
            return None;
        }
        self.bodies
            .iter()
            .find(|(id, _)| *id == part)
            .map(|(_, body)| body)
    }

    pub(crate) fn set_body(&mut self, part: PartId, body: BodyEdit<'x>) {
        match self.bodies.iter_mut().find(|(id, _)| *id == part) {
            Some((_, edited)) => *edited = body,
            None => self.bodies.push((part, body)),
        }
    }

    pub(crate) fn hide(&mut self, range: Range<PartId>) {
        if range.is_empty() {
            return;
        }
        self.headers.retain(|(id, _)| !range.contains(id));
        self.bodies.retain(|(id, _)| !range.contains(id));
        self.hidden.retain(|hidden| !range.contains(&hidden.start));
        self.hidden.push(range);
    }

    #[inline(always)]
    pub(crate) fn hidden_end(&self, part: PartId) -> Option<PartId> {
        if self.hidden.is_empty() {
            return None;
        }
        self.hidden
            .iter()
            .find(|hidden| hidden.contains(&part))
            .map(|hidden| hidden.end)
    }

    pub(crate) fn touches(&self, range: &Range<PartId>) -> bool {
        self.headers.iter().any(|(id, _)| range.contains(id))
            || self.bodies.iter().any(|(id, _)| range.contains(id))
    }
}
