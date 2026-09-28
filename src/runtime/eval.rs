/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::message::{body::TextForm, headers::HeaderRef};
use super::{RuntimeError, Variable};
use crate::{
    Context, Envelope, Sieve,
    bytecode::{
        Corrupt, Decoded,
        rec::{Range, Rec, tag},
    },
    compiler::{ReceivedHostname, ReceivedPart, grammar::AddressPart},
};
use bumpalo::Bump;
use mail_parser::{
    ContentType, HeaderForm, HeaderName, HeaderValue, Host, Mailbox, PartKind, Received,
};
use smallvec::SmallVec;
use std::cmp::Ordering;

pub(crate) type Values<'x> = SmallVec<[Variable<'x>; 4]>;

#[derive(Debug, Clone, Copy)]
pub(crate) enum ValueRef<'x> {
    Text(&'x str),
    Int(i64),
    Float(f64),
    Local(u16),
    Match(u8),
    Global(&'x str),
    Env(&'x str),
    Envelope(Envelope),
    Part { kind: u8, convert: bool },
    Header(HeaderVar<'x>),
    Regex { pattern: &'x str },
    Glob { pattern: &'x str },
    HeaderName(&'x HeaderName<'static>),
    List(Range),
    None,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HeaderVar<'x> {
    pub names: Range,
    pub part: HeaderPartRef<'x>,
    pub index_hdr: i32,
    pub index_part: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum HeaderPartRef<'x> {
    Text,
    Date,
    Id,
    Address(AddressPart),
    ContentType(ContentTypeRef<'x>),
    Received(ReceivedPart),
    Raw,
    RawName,
    Exists,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContentTypeRef<'x> {
    Type,
    Subtype,
    Attribute(&'x str),
}

impl<'x> ValueRef<'x> {
    pub(crate) fn decode(
        script: &'x Sieve<'x>,
        rec: Rec,
        following: &mut impl Iterator<Item = Rec>,
    ) -> Decoded<ValueRef<'x>> {
        Ok(match rec.tag {
            tag::TEXT => ValueRef::Text(script.str(rec.str())?),
            tag::INT => ValueRef::Int(rec.e as i64),
            tag::FLOAT => ValueRef::Float(f64::from_bits(rec.e)),
            tag::VAR_LOCAL => ValueRef::Local(rec.c),
            tag::VAR_MATCH => ValueRef::Match(rec.b),
            tag::VAR_GLOBAL => ValueRef::Global(script.str(rec.str())?),
            tag::VAR_ENV => ValueRef::Env(script.str(rec.str())?),
            tag::VAR_ENVELOPE | tag::ENVELOPE => ValueRef::Envelope(Envelope::from_code(rec.b)),
            tag::VAR_PART => ValueRef::Part {
                kind: rec.b,
                convert: rec.c != 0,
            },
            tag::VAR_HEADER => {
                let cont = following.next().ok_or(Corrupt)?;
                if cont.tag != tag::CONT {
                    return Err(Corrupt);
                }
                let attr = if cont.e != 0 {
                    script.str(cont.str())?
                } else {
                    ""
                };
                ValueRef::Header(HeaderVar {
                    names: Range {
                        start: rec.d,
                        len: cont.c as u32,
                    },
                    part: HeaderPartRef::decode(rec.b, rec.c, attr),
                    index_hdr: (rec.e >> 32) as u32 as i32,
                    index_part: rec.e as u32 as i32,
                })
            }
            tag::REF => {
                let target = script.rec(rec.d)?;
                let mut iter = script.recs(Range {
                    start: rec.d + 1,
                    len: rec.e as u32,
                })?;
                return ValueRef::decode(script, target, &mut iter);
            }
            tag::REGEX => ValueRef::Regex {
                pattern: script.str(rec.str())?,
            },
            tag::GLOB => ValueRef::Glob {
                pattern: script.str(rec.str())?,
            },
            tag::HEADER => ValueRef::HeaderName(script.header_name(rec.c)?),
            tag::LIST => ValueRef::List(rec.range()),
            tag::NONE | tag::VARIABLE_NONE => ValueRef::None,
            _ => return Err(Corrupt),
        })
    }
}

impl ContentTypeRef<'_> {
    fn select<'y>(&self, ct: ContentType<'y>) -> Option<&'y str> {
        match self {
            ContentTypeRef::Type => Some(ct.ctype()),
            ContentTypeRef::Subtype => ct.subtype(),
            ContentTypeRef::Attribute(attr) => ct
                .attributes()
                .find(|(name, _)| name.eq_ignore_ascii_case(attr))
                .map(|(_, value)| value),
        }
    }
}

impl<'x> HeaderPartRef<'x> {
    fn decode(part: u8, sub: u16, attr: &'x str) -> Self {
        match part {
            0 => HeaderPartRef::Text,
            1 => HeaderPartRef::Date,
            2 => HeaderPartRef::Id,
            3 => HeaderPartRef::Address(AddressPart::from_code(sub as u8)),
            4 => HeaderPartRef::ContentType(match sub as u8 {
                0 => ContentTypeRef::Type,
                1 => ContentTypeRef::Subtype,
                _ => ContentTypeRef::Attribute(attr),
            }),
            5 => HeaderPartRef::Received(ReceivedPart::from_code(
                (sub & 0xff) as u8,
                (sub >> 8) as u8,
            )),
            6 => HeaderPartRef::Raw,
            7 => HeaderPartRef::RawName,
            _ => HeaderPartRef::Exists,
        }
    }
}

impl<'x> Context<'x> {
    #[inline(always)]
    pub(crate) fn eval_value(
        &self,
        script: &'x Sieve<'x>,
        rec: Rec,
    ) -> Result<Variable<'x>, RuntimeError> {
        match rec.tag {
            tag::TEXT => Ok(Variable::borrowed(script.str(rec.str())?)),
            tag::VAR_LOCAL => Ok(self.local_variable(rec.c)),
            _ => {
                let value = ValueRef::decode(script, rec, &mut std::iter::empty())?;
                self.eval_value_ref(script, value)
            }
        }
    }

    pub(crate) fn eval_value_ref(
        &self,
        script: &'x Sieve<'x>,
        value: ValueRef<'x>,
    ) -> Result<Variable<'x>, RuntimeError> {
        Ok(match value {
            ValueRef::Text(text) => Variable::borrowed(text),
            ValueRef::Int(n) => Variable::Integer(n),
            ValueRef::Float(n) => Variable::Float(n),
            ValueRef::List(range) => {
                let mut data = ArenaString::new_in(&self.arena.bump);
                let mut iter = script.recs(range)?;
                while let Some(item) = iter.next() {
                    match item.tag {
                        tag::TEXT => data.push_str(script.str(item.str())?)?,
                        tag::VAR_LOCAL => {
                            if let Some(value) =
                                self.vars_local.get(self.local_base() + item.c as usize)
                            {
                                data.push_variable(value)?;
                            }
                        }
                        tag::INT => data.push_str(&(item.e as i64).to_string())?,
                        tag::FLOAT => data.push_str(&f64::from_bits(item.e).to_string())?,
                        tag::REGEX | tag::GLOB => (),
                        tag::HEADER => data.push_str(script.header_name(item.c)?.as_str())?,
                        _ => {
                            let item = ValueRef::decode(script, item, &mut iter)?;
                            if let Some(value) = self.variable_ref(script, item)? {
                                data.push_variable(&value)?;
                            }
                        }
                    }
                }
                Variable::borrowed(unsafe { super::context::extend(data.into_str()) })
            }
            ValueRef::Regex { pattern, .. } | ValueRef::Glob { pattern, .. } => {
                Variable::borrowed(pattern)
            }
            ValueRef::HeaderName(name) => Variable::borrowed(name.as_str()),
            ValueRef::None => Variable::default(),
            other => self.variable_ref(script, other)?.unwrap_or_default(),
        })
    }

    #[inline(always)]
    pub(crate) fn local_variable(&self, id: u16) -> Variable<'x> {
        self.vars_local
            .get(self.local_base() + id as usize)
            .cloned()
            .unwrap_or_default()
    }

    #[inline(always)]
    pub(crate) fn match_variable(&self, id: u8) -> Variable<'x> {
        self.vars_match
            .get(self.match_base() + id as usize)
            .cloned()
            .unwrap_or_default()
    }

    pub(crate) fn variable_ref(
        &self,
        script: &'x Sieve<'x>,
        value: ValueRef<'x>,
    ) -> Result<Option<Variable<'x>>, RuntimeError> {
        Ok(match value {
            ValueRef::Local(id) => self
                .vars_local
                .get(self.local_base() + id as usize)
                .cloned(),
            ValueRef::Match(id) => self
                .vars_match
                .get(self.match_base() + id as usize)
                .cloned(),
            ValueRef::Global(name) => self.vars_global.get(name).cloned(),
            ValueRef::Env(name) => self
                .vars_env
                .get(name)
                .or_else(|| self.runtime.environment.get(name))
                .cloned(),
            ValueRef::Envelope(envelope) => self.envelope.iter().find_map(|(e, v)| {
                if *e == envelope {
                    Some(v.clone())
                } else {
                    None
                }
            }),
            ValueRef::Header(header) => self.eval_header(script, &header)?,
            ValueRef::Part { kind, convert } => self.eval_part(kind, convert),
            ValueRef::Text(text) => Some(Variable::borrowed(text)),
            ValueRef::Int(n) => Some(Variable::Integer(n)),
            ValueRef::Float(n) => Some(Variable::Float(n)),
            other => Some(self.eval_value_ref(script, other)?),
        })
    }

    fn eval_part(&self, kind: u8, convert: bool) -> Option<Variable<'x>> {
        let text = match kind {
            0 => {
                let part = self.message.root().text_body().next()?;
                match self.part_kind(part) {
                    PartKind::Text => self.part_text(part, TextForm::Source),
                    PartKind::Html if convert => self.part_text(part, TextForm::Plain),
                    _ => None,
                }
            }
            1 => {
                let part = self.message.root().html_body().next()?;
                match self.part_kind(part) {
                    PartKind::Html => self.part_text(part, TextForm::Source),
                    PartKind::Text if convert => self.part_text(part, TextForm::Html),
                    _ => None,
                }
            }
            2 => {
                let part = self.current_part()?;
                match self.part_kind(part) {
                    PartKind::Text | PartKind::Html => self.part_text(part, TextForm::Source),
                    PartKind::Binary | PartKind::InlineBinary => {
                        self.part_text(part, TextForm::Decoded)
                    }
                    _ => None,
                }
            }
            _ => self
                .current_part()
                .and_then(|part| self.part_text(part, TextForm::Raw)),
        };
        text.map(Variable::borrowed)
    }

    pub(crate) fn eval_values(
        &self,
        script: &'x Sieve<'x>,
        range: Range,
    ) -> Result<Values<'x>, RuntimeError> {
        let mut result = Values::with_capacity(range.len as usize);
        let mut iter = script.recs(range)?;
        while let Some(rec) = iter.next() {
            let value = ValueRef::decode(script, rec, &mut iter)?;
            result.push(self.eval_value_ref(script, value)?);
        }
        Ok(result)
    }

    pub(crate) fn eval_strings(
        &self,
        script: &'x Sieve<'x>,
        range: Range,
    ) -> Result<SmallVec<[&'x str; 4]>, RuntimeError> {
        let mut result = SmallVec::with_capacity(range.len as usize);
        let mut iter = script.recs(range)?;
        while let Some(rec) = iter.next() {
            let value = ValueRef::decode(script, rec, &mut iter)?;
            let value = self.eval_value_ref(script, value)?;
            result.push(self.intern_cow(value.into_string()));
        }
        Ok(result)
    }

    pub(crate) fn eval_opt(
        &self,
        script: &'x Sieve<'x>,
        rec: Rec,
    ) -> Result<Option<Variable<'x>>, RuntimeError> {
        if rec.tag == tag::NONE {
            Ok(None)
        } else {
            self.eval_value(script, rec).map(Some)
        }
    }

    pub(crate) fn eval_str(
        &self,
        script: &'x Sieve<'x>,
        rec: Rec,
    ) -> Result<&'x str, RuntimeError> {
        Ok(self.intern_cow(self.eval_value(script, rec)?.into_string()))
    }

    pub(crate) fn eval_opt_str(
        &self,
        script: &'x Sieve<'x>,
        rec: Rec,
    ) -> Result<Option<&'x str>, RuntimeError> {
        if rec.tag == tag::NONE {
            Ok(None)
        } else {
            self.eval_str(script, rec).map(Some)
        }
    }

    fn eval_header(
        &self,
        script: &'x Sieve<'x>,
        header: &HeaderVar<'x>,
    ) -> Result<Option<Variable<'x>>, RuntimeError> {
        let mut result: Values<'x> = SmallVec::new();
        let Some(part) = self.current_part() else {
            return Ok(None);
        };
        if !header.names.is_empty() {
            let mut names: SmallVec<[&HeaderName<'static>; 2]> = SmallVec::new();
            for rec in script.recs(header.names)? {
                names.push(script.header_name(rec.c)?);
            }
            match (header.index_hdr.cmp(&0), names.as_slice()) {
                (Ordering::Less, [name]) if header.index_hdr == -1 => {
                    if let Some(h) = self.last_header(part, name) {
                        self.eval_header_part(header, h, &mut result);
                    }
                }
                (Ordering::Less, [name]) => {
                    if let Some(h) = self
                        .part_headers(part)
                        .rev()
                        .filter(|h| h.is_named(name))
                        .nth((header.index_hdr.unsigned_abs() - 1) as usize)
                    {
                        self.eval_header_part(header, h, &mut result);
                    }
                }
                (Ordering::Greater, [name]) => {
                    if let Some(h) = self
                        .named_headers(part, name)
                        .nth((header.index_hdr - 1) as usize)
                    {
                        self.eval_header_part(header, h, &mut result);
                    }
                }
                (Ordering::Equal, [name]) => {
                    for h in self.named_headers(part, name) {
                        self.eval_header_part(header, h, &mut result);
                    }
                }
                (index, names) => {
                    let matching = self.matching_headers(part, names);
                    let selected = match index {
                        Ordering::Greater => matching.get((header.index_hdr - 1) as usize),
                        Ordering::Less => matching
                            .iter()
                            .rev()
                            .nth((header.index_hdr.unsigned_abs() - 1) as usize),
                        Ordering::Equal => {
                            for h in &matching {
                                self.eval_header_part(header, *h, &mut result);
                            }
                            None
                        }
                    };
                    if let Some(h) = selected {
                        self.eval_header_part(header, *h, &mut result);
                    }
                }
            }
        } else {
            let source = part.message().source_bytes();
            for h in self.part_headers(part) {
                match &header.part {
                    HeaderPartRef::Raw => {
                        let field = match h {
                            HeaderRef::Added(_) => &[][..],
                            h => raw_field(source, h),
                        };
                        result.push(Variable::borrowed(
                            self.alloc_string(sanitize_raw_header(field)),
                        ));
                    }
                    HeaderPartRef::Text => {
                        let parsed;
                        let text = match h.value() {
                            HeaderValue::Text(text) => Some(text),
                            _ => {
                                parsed = h.parse_as(HeaderForm::Text);
                                parsed.value().as_text()
                            }
                        };
                        if let Some(text) = text {
                            result.push(Variable::borrowed(self.alloc_string(format!(
                                "{}: {}",
                                h.name().as_str(),
                                text
                            ))));
                        }
                    }
                    _ => {
                        self.eval_header_part(header, h, &mut result);
                    }
                }
            }
        }

        match result.len() {
            1 if header.index_hdr != 0 && header.index_part != 0 => Ok(result.pop()),
            0 => Ok(None),
            _ => self
                .try_alloc_variables(result)
                .map(|items| Some(Variable::Array(super::variable::Array::Borrowed(items)))),
        }
    }

    fn eval_header_part(&self, header: &HeaderVar<'x>, h: HeaderRef<'x>, result: &mut Values<'x>) {
        let index_part = header.index_part;
        let var = match &header.part {
            HeaderPartRef::Text => match h.value() {
                HeaderValue::Text(v) if [-1, 0, 1].contains(&index_part) => {
                    Some(Variable::borrowed(v))
                }
                HeaderValue::TextList(list) => {
                    match pick(list.iter(), index_part, result, |item| {
                        Some(Variable::borrowed(item))
                    }) {
                        Some(var) => var,
                        None => return,
                    }
                }
                HeaderValue::ContentType(ct) => Some(Variable::borrowed(match ct.subtype() {
                    Some(subtype) => self.alloc_string(format!("{}/{}", ct.ctype(), subtype)),
                    None => ct.ctype(),
                })),
                HeaderValue::Address(list) => {
                    match pick(list.mailboxes(), index_part, result, |mailbox| {
                        Some(self.addr_to_text(mailbox))
                    }) {
                        Some(var) => var,
                        None => return,
                    }
                }
                HeaderValue::DateTime(_) => h
                    .raw_value()
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .map(|s| Variable::borrowed(s.trim())),
                _ => None,
            },
            HeaderPartRef::Address(part) => match h.value() {
                HeaderValue::Address(list) => {
                    match pick(list.mailboxes(), index_part, result, |mailbox| {
                        self.addr_part(part, mailbox)
                    }) {
                        Some(var) => var,
                        None => return,
                    }
                }
                HeaderValue::Text(_) => {
                    let parsed = h.parse_as(HeaderForm::Addresses);
                    let Some(list) = parsed.value().as_address() else {
                        result.push(Variable::default());
                        return;
                    };
                    match pick(list.mailboxes(), index_part, result, |mailbox| {
                        part.eval_strict(mailbox)
                            .map(|s| Variable::borrowed(self.alloc_str(s)))
                    }) {
                        Some(var) => var,
                        None => return,
                    }
                }
                _ => None,
            },
            HeaderPartRef::Date => match h.value() {
                HeaderValue::DateTime(dt) => Some(Variable::from(dt.to_timestamp())),
                _ => h
                    .parse_as(HeaderForm::Date)
                    .value()
                    .as_datetime()
                    .map(|dt| Variable::from(dt.to_timestamp())),
            },
            HeaderPartRef::Id => {
                let name = h.name();
                if matches!(name, HeaderName::MessageId | HeaderName::ResentMessageId) {
                    match h.value() {
                        HeaderValue::Text(id) => Some(Variable::borrowed(id)),
                        HeaderValue::TextList(ids) => {
                            result.extend(ids.iter().map(Variable::borrowed));
                            return;
                        }
                        _ => None,
                    }
                } else if name.is_structured() {
                    None
                } else {
                    let parsed = h.parse_as(HeaderForm::MessageIds);
                    match parsed.value() {
                        HeaderValue::Text(id) => Some(Variable::borrowed(self.alloc_str(id))),
                        HeaderValue::TextList(ids) if !ids.is_empty() => {
                            result.extend(
                                ids.iter().map(|id| Variable::borrowed(self.alloc_str(id))),
                            );
                            return;
                        }
                        _ => None,
                    }
                }
            }
            HeaderPartRef::Raw => Some(Variable::borrowed(
                self.alloc_string(sanitize_raw_header(h.raw_value().unwrap_or_default())),
            )),
            HeaderPartRef::RawName => match h {
                HeaderRef::Added(_) => None,
                h => Some(Variable::borrowed(h.raw_name())),
            },
            HeaderPartRef::Exists => Some(Variable::from(true)),
            HeaderPartRef::ContentType(part) => match h {
                HeaderRef::Added(_) => h
                    .parse_as(HeaderForm::ContentType)
                    .value()
                    .as_content_type()
                    .and_then(|ct| part.select(ct))
                    .map(|value| Variable::borrowed(self.alloc_str(value))),
                h => h
                    .value()
                    .as_content_type()
                    .and_then(|ct| part.select(ct))
                    .map(Variable::borrowed),
            },
            HeaderPartRef::Received(part) => match h.value() {
                HeaderValue::Received(rcvd) => self.received_part(part, rcvd),
                _ => None,
            },
        };

        result.push(var.unwrap_or_default());
    }

    fn addr_to_text(&self, mailbox: Mailbox<'x>) -> Variable<'x> {
        match (mailbox.name(), mailbox.address()) {
            (Some(name), Some(address)) => {
                Variable::borrowed(self.alloc_string(format!("{name} <{address}>")))
            }
            (Some(name), None) => Variable::borrowed(name),
            (None, Some(address)) => Variable::borrowed(self.alloc_string(format!("<{address}>"))),
            (None, None) => Variable::default(),
        }
    }

    fn addr_part(&self, part: &AddressPart, mailbox: Mailbox<'x>) -> Option<Variable<'x>> {
        match part {
            AddressPart::Name => mailbox.name().map(Variable::borrowed),
            AddressPart::All => mailbox.address().map(Variable::borrowed),
            _ => part.eval_strict(mailbox).map(Variable::borrowed),
        }
    }

    pub fn received_part(&self, part: &ReceivedPart, rcvd: Received<'x>) -> Option<Variable<'x>> {
        match part {
            ReceivedPart::From(from) => rcvd
                .from()
                .or_else(|| rcvd.helo())
                .and_then(|host| self.host_variable(from, host)),
            ReceivedPart::FromIp => rcvd
                .from_ip()
                .map(|ip| Variable::borrowed(self.alloc_string(ip.to_string()))),
            ReceivedPart::FromIpRev => rcvd.from_iprev().map(Variable::borrowed),
            ReceivedPart::By(by) => rcvd.by().and_then(|host| self.host_variable(by, host)),
            ReceivedPart::For => rcvd.for_().map(Variable::borrowed),
            ReceivedPart::With => rcvd.with().map(|v| Variable::borrowed(v.as_str())),
            ReceivedPart::TlsVersion => rcvd.tls_version().map(|v| Variable::borrowed(v.as_str())),
            ReceivedPart::TlsCipher => rcvd.tls_cipher().map(Variable::borrowed),
            ReceivedPart::Id => rcvd.id().map(Variable::borrowed),
            ReceivedPart::Ident => rcvd.ident().map(Variable::borrowed),
            ReceivedPart::Via => rcvd.via().map(Variable::borrowed),
            ReceivedPart::Date => rcvd.date().map(|d| Variable::from(d.to_timestamp())),
            ReceivedPart::DateRaw => rcvd
                .date()
                .map(|d| Variable::borrowed(self.alloc_string(d.to_rfc822()))),
        }
    }

    fn host_variable(&self, hostname: &ReceivedHostname, host: Host<'x>) -> Option<Variable<'x>> {
        match (hostname, host) {
            (ReceivedHostname::Name | ReceivedHostname::Any, Host::Name(name)) => {
                Some(Variable::borrowed(name))
            }
            (ReceivedHostname::Ip | ReceivedHostname::Any, Host::IpAddr(ip)) => {
                Some(Variable::borrowed(self.alloc_string(ip.to_string())))
            }
            _ => None,
        }
    }
}

fn raw_field<'x>(source: &'x [u8], header: HeaderRef<'x>) -> &'x [u8] {
    match header {
        HeaderRef::Parsed(header) | HeaderRef::Renamed(header, _) => source
            .get(header.offset_field() as usize..header.offset_end() as usize)
            .unwrap_or_default(),
        HeaderRef::Added(_) => &[],
    }
}

fn pick<'x, T>(
    mut items: impl DoubleEndedIterator<Item = T>,
    index: i32,
    result: &mut Values<'x>,
    mut map: impl FnMut(T) -> Option<Variable<'x>>,
) -> Option<Option<Variable<'x>>> {
    match index.cmp(&0) {
        Ordering::Greater => Some(items.nth((index - 1) as usize).and_then(map)),
        Ordering::Less => Some(
            items
                .rev()
                .nth((index.unsigned_abs() - 1) as usize)
                .and_then(map),
        ),
        Ordering::Equal => {
            result.extend(items.map(|item| map(item).unwrap_or_default()));
            None
        }
    }
}

struct ArenaString<'a> {
    bytes: bumpalo::collections::Vec<'a, u8>,
}

impl<'a> ArenaString<'a> {
    #[inline(always)]
    fn new_in(arena: &'a Bump) -> Self {
        ArenaString {
            bytes: bumpalo::collections::Vec::new_in(arena),
        }
    }

    #[inline(always)]
    fn push_str(&mut self, s: &str) -> Result<(), RuntimeError> {
        self.bytes
            .try_reserve(s.len())
            .map_err(|_| RuntimeError::MemoryLimitReached)?;
        self.bytes.extend_from_slice(s.as_bytes());
        Ok(())
    }

    fn push_variable(&mut self, value: &Variable<'_>) -> Result<(), RuntimeError> {
        match value {
            Variable::String(s) => self.push_str(s),
            Variable::Integer(n) => self.push_str(&n.to_string()),
            Variable::Float(n) => self.push_str(&n.to_string()),
            Variable::Array(items) => self.push_str(&super::variable::array_to_string(items)),
        }
    }

    #[inline(always)]
    fn into_str(self) -> &'a str {
        unsafe { std::str::from_utf8_unchecked(self.bytes.into_bump_slice()) }
    }
}

pub(crate) trait IntoString: Sized {
    fn into_string(self) -> String;
}

impl IntoString for Vec<u8> {
    fn into_string(self) -> String {
        String::from_utf8(self)
            .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
    }
}

pub(crate) fn sanitize_raw_header(bytes: &[u8]) -> String {
    let mut result = Vec::with_capacity(bytes.len());
    let mut last_is_space = false;

    for &ch in bytes {
        if ch.is_ascii_whitespace() {
            last_is_space = true;
        } else {
            if last_is_space {
                result.push(b' ');
                last_is_space = false;
            }
            result.push(ch);
        }
    }

    result.into_string()
}
