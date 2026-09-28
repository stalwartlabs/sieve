/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::Context;
use mail_parser::{
    Header, HeaderForm, HeaderIter, HeaderKey, HeaderName, HeaderValue, MessagePart, NamedHeaders,
    ParsedValue,
};
use smallvec::SmallVec;
use std::slice;

pub(crate) static ORIGINAL_SUBJECT: HeaderName<'static> = HeaderName::OriginalSubject;
pub(crate) static ORIGINAL_FROM: HeaderName<'static> = HeaderName::OriginalFrom;

pub(crate) struct AddedHeader<'x> {
    pub(crate) name: HeaderName<'x>,
    pub(crate) value: &'x str,
    content_type: Option<(&'x str, &'x str)>,
}

#[derive(Clone, Copy)]
pub(crate) enum HeaderRef<'x> {
    Parsed(Header<'x>),
    Renamed(Header<'x>, &'x HeaderName<'x>),
    Added(&'x AddedHeader<'x>),
}

impl<'x> HeaderRef<'x> {
    pub(crate) fn name(&self) -> HeaderName<'x> {
        match self {
            HeaderRef::Parsed(header) => header.name(),
            HeaderRef::Renamed(_, name) => (*name).clone(),
            HeaderRef::Added(added) => added.name.clone(),
        }
    }

    pub(crate) fn is_named(&self, name: &HeaderName<'_>) -> bool {
        match self {
            HeaderRef::Parsed(header) => header.name() == *name,
            HeaderRef::Renamed(_, renamed) => *renamed == name,
            HeaderRef::Added(added) => added.name == *name,
        }
    }

    pub(crate) fn value(&self) -> HeaderValue<'x> {
        match self {
            HeaderRef::Parsed(header) | HeaderRef::Renamed(header, _) => header.value(),
            HeaderRef::Added(added) => HeaderValue::Text(added.value),
        }
    }

    pub(crate) fn parse_as(&self, form: HeaderForm) -> ParsedValue<'x> {
        match self {
            HeaderRef::Parsed(header) | HeaderRef::Renamed(header, _) => header.parse_as(form),
            HeaderRef::Added(added) => form.parse(added.value.as_bytes()),
        }
    }

    fn position(&self) -> u32 {
        match self {
            HeaderRef::Parsed(header) | HeaderRef::Renamed(header, _) => header.offset_field(),
            HeaderRef::Added(_) => u32::MAX,
        }
    }

    pub(crate) fn raw_value(&self) -> Option<&'x [u8]> {
        match self {
            HeaderRef::Parsed(header) | HeaderRef::Renamed(header, _) => Some(header.raw_value()),
            HeaderRef::Added(_) => None,
        }
    }

    pub(crate) fn raw_name(&self) -> &'x str {
        match self {
            HeaderRef::Parsed(header) => header.raw_name(),
            HeaderRef::Renamed(_, name) => name.as_str(),
            HeaderRef::Added(added) => added.name.as_str(),
        }
    }

    pub(crate) fn len(&self) -> usize {
        match self {
            HeaderRef::Parsed(header) => {
                header.offset_end().saturating_sub(header.offset_field()) as usize
            }
            HeaderRef::Renamed(header, name) => name.as_str().len() + 1 + header.raw_value().len(),
            HeaderRef::Added(added) => added.name.as_str().len() + added.value.len() + 4,
        }
    }

    pub(crate) fn same(&self, other: &HeaderRef<'_>) -> bool {
        match (self, other) {
            (
                HeaderRef::Parsed(a) | HeaderRef::Renamed(a, _),
                HeaderRef::Parsed(b) | HeaderRef::Renamed(b, _),
            ) => a.offset_field() == b.offset_field() && a.offset_end() == b.offset_end(),
            (HeaderRef::Added(a), HeaderRef::Added(b)) => std::ptr::eq(*a, *b),
            _ => false,
        }
    }

    pub(crate) fn write(&self, source: &[u8], out: &mut Vec<u8>) {
        match self {
            HeaderRef::Parsed(header) => out.extend_from_slice(
                source
                    .get(header.offset_field() as usize..header.offset_end() as usize)
                    .unwrap_or_default(),
            ),
            HeaderRef::Renamed(header, name) => {
                out.extend_from_slice(name.as_str().as_bytes());
                out.push(b':');
                out.extend_from_slice(header.raw_value());
            }
            HeaderRef::Added(added) => {
                out.extend_from_slice(added.name.as_str().as_bytes());
                out.extend_from_slice(b": ");
                out.extend_from_slice(added.value.as_bytes());
                out.extend_from_slice(b"\r\n");
            }
        }
    }
}

pub(crate) enum PartHeaders<'y, 'x> {
    Parsed(HeaderIter<'x>),
    Edited(slice::Iter<'y, HeaderRef<'x>>),
}

impl<'x> Iterator for PartHeaders<'_, 'x> {
    type Item = HeaderRef<'x>;

    #[inline]
    fn next(&mut self) -> Option<HeaderRef<'x>> {
        match self {
            PartHeaders::Parsed(iter) => iter.next().map(HeaderRef::Parsed),
            PartHeaders::Edited(iter) => iter.next().copied(),
        }
    }
}

impl<'x> DoubleEndedIterator for PartHeaders<'_, 'x> {
    #[inline]
    fn next_back(&mut self) -> Option<HeaderRef<'x>> {
        match self {
            PartHeaders::Parsed(iter) => iter.next_back().map(HeaderRef::Parsed),
            PartHeaders::Edited(iter) => iter.next_back().copied(),
        }
    }
}

pub(crate) enum NamedPartHeaders<'y, 'x> {
    Parsed(NamedHeaders<'x, 'y>),
    Edited(slice::Iter<'y, HeaderRef<'x>>, &'y HeaderName<'y>),
}

impl<'x> Iterator for NamedPartHeaders<'_, 'x> {
    type Item = HeaderRef<'x>;

    #[inline]
    fn next(&mut self) -> Option<HeaderRef<'x>> {
        match self {
            NamedPartHeaders::Parsed(iter) => iter.next().map(HeaderRef::Parsed),
            NamedPartHeaders::Edited(iter, name) => {
                iter.find(|header| header.is_named(name)).copied()
            }
        }
    }
}

impl<'x> Context<'x> {
    #[inline]
    pub(crate) fn part_headers(&self, part: MessagePart<'x>) -> PartHeaders<'_, 'x> {
        match self.edits.headers(part.id()) {
            Some(headers) => PartHeaders::Edited(headers.iter()),
            None => PartHeaders::Parsed(part.headers().iter()),
        }
    }

    #[inline]
    pub(crate) fn named_headers<'y>(
        &'y self,
        part: MessagePart<'x>,
        name: &'y HeaderName<'y>,
    ) -> NamedPartHeaders<'y, 'x> {
        self.keyed_headers(part, name, name.key())
    }

    #[inline]
    pub(crate) fn keyed_headers<'y>(
        &'y self,
        part: MessagePart<'x>,
        name: &'y HeaderName<'y>,
        key: HeaderKey<'y>,
    ) -> NamedPartHeaders<'y, 'x> {
        match self.edits.headers(part.id()) {
            Some(headers) => NamedPartHeaders::Edited(headers.iter(), name),
            None => NamedPartHeaders::Parsed(part.headers().all_key(key)),
        }
    }

    pub(crate) fn matching_headers(
        &self,
        part: MessagePart<'x>,
        names: &[&HeaderName<'_>],
    ) -> SmallVec<[HeaderRef<'x>; 8]> {
        let mut matching: SmallVec<[HeaderRef<'x>; 8]> = SmallVec::new();
        if self.edits.headers(part.id()).is_some() {
            matching.extend(self.part_headers(part).filter(|header| {
                let name = header.name();
                names.iter().any(|wanted| **wanted == name)
            }));
        } else {
            let headers = part.headers();
            for name in names {
                matching.extend(headers.all_key(name.key()).map(HeaderRef::Parsed));
            }
            matching.sort_unstable_by_key(HeaderRef::position);
            matching.dedup_by_key(|header| header.position());
        }
        matching
    }

    pub(crate) fn root_headers(&self) -> PartHeaders<'_, 'x> {
        match self.root_part() {
            Some(part) => self.part_headers(part),
            None => PartHeaders::Edited([].iter()),
        }
    }

    pub(crate) fn last_header(
        &self,
        part: MessagePart<'x>,
        name: &HeaderName<'_>,
    ) -> Option<HeaderRef<'x>> {
        match self.edits.headers(part.id()) {
            Some(headers) => headers
                .iter()
                .rev()
                .find(|header| header.is_named(name))
                .copied(),
            None => part.headers().get_key(name.key()).map(HeaderRef::Parsed),
        }
    }

    pub(crate) fn root_text(&self, name: &HeaderName<'_>) -> Option<&'x str> {
        self.root_part()
            .and_then(|part| self.last_header(part, name))
            .and_then(|header| header.value().as_text())
    }

    pub(crate) fn part_content_type(&self, part: MessagePart<'x>) -> Option<(&'x str, &'x str)> {
        if self.edits.headers(part.id()).is_none() {
            return part
                .content_type()
                .map(|ct| (ct.ctype(), ct.subtype().unwrap_or_default()));
        }
        match self.last_header(part, &HeaderName::ContentType)? {
            HeaderRef::Added(added) => added.content_type,
            header => header
                .value()
                .as_content_type()
                .map(|ct| (ct.ctype(), ct.subtype().unwrap_or_default())),
        }
    }

    pub(crate) fn renamed_header(
        &self,
        header: HeaderRef<'x>,
        name: &'x HeaderName<'x>,
    ) -> HeaderRef<'x> {
        match header {
            HeaderRef::Parsed(parsed) | HeaderRef::Renamed(parsed, _) => {
                HeaderRef::Renamed(parsed, name)
            }
            HeaderRef::Added(added) => self.add_header(name.clone(), added.value),
        }
    }

    pub(crate) fn add_header(&self, name: HeaderName<'x>, value: &'x str) -> HeaderRef<'x> {
        let content_type = if name == HeaderName::ContentType {
            HeaderForm::ContentType
                .parse(value.as_bytes())
                .value()
                .as_content_type()
                .map(|ct| {
                    (
                        self.alloc_str(ct.ctype()),
                        self.alloc_str(ct.subtype().unwrap_or_default()),
                    )
                })
        } else {
            None
        };
        match self.arena.bump.try_alloc(AddedHeader {
            name,
            value,
            content_type,
        }) {
            Ok(added) => HeaderRef::Added(unsafe { super::super::context::extend(added) }),
            Err(_) => {
                self.note_oom();
                HeaderRef::Added(&EMPTY_HEADER)
            }
        }
    }
}

static EMPTY_HEADER: AddedHeader<'static> = AddedHeader {
    name: HeaderName::Other(std::borrow::Cow::Borrowed("")),
    value: "",
    content_type: None,
};
