/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

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
    Addr, Header, HeaderName, HeaderValue, Host, PartType, Received,
    decoders::html::{html_to_text, text_to_html},
    parsers::MessageStream,
};
use smallvec::SmallVec;
use std::{borrow::Cow, cmp::Ordering};

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
    pub(crate) fn cow_str(&self, s: &Cow<'x, str>) -> &'x str {
        match s {
            Cow::Borrowed(s) => s,
            Cow::Owned(s) => self.alloc_str(s),
        }
    }

    #[inline(always)]
    pub(crate) fn raw_message(&self) -> Result<&'x [u8], RuntimeError> {
        match &self.message.raw_message {
            Cow::Borrowed(raw) => Ok(raw),
            Cow::Owned(raw) => match self.raw_message_copy.get() {
                Some(copy) => Ok(copy),
                None => {
                    let copy = self
                        .arena
                        .bump
                        .try_alloc_slice_copy(raw)
                        .map(|bytes| unsafe { super::context::extend(bytes) })
                        .map_err(|_| RuntimeError::MemoryLimitReached)?;
                    self.raw_message_copy.set(Some(copy));
                    Ok(copy)
                }
            },
        }
    }

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
        match kind {
            0 => {
                let part = self
                    .message
                    .parts
                    .get(*self.message.text_body.first()? as usize)?;
                match &part.body {
                    PartType::Text(text) => Some(Variable::borrowed(self.cow_str(text))),
                    PartType::Html(html) if convert => Some(Variable::borrowed(
                        self.alloc_string(html_to_text(html.as_ref())),
                    )),
                    _ => None,
                }
            }
            1 => {
                let part = self
                    .message
                    .parts
                    .get(*self.message.html_body.first()? as usize)?;
                match &part.body {
                    PartType::Html(html) => Some(Variable::borrowed(self.cow_str(html))),
                    PartType::Text(text) if convert => Some(Variable::borrowed(
                        self.alloc_string(text_to_html(text.as_ref())),
                    )),
                    _ => None,
                }
            }
            2 => match &self.message.parts.get(self.part as usize)?.body {
                PartType::Text(text) | PartType::Html(text) => {
                    Some(Variable::borrowed(self.cow_str(text)))
                }
                PartType::Binary(bin) | PartType::InlineBinary(bin) => Some(Variable::borrowed(
                    self.alloc_str(&String::from_utf8_lossy(bin.as_ref())),
                )),
                _ => None,
            },
            _ => {
                let part = self.message.parts.get(self.part as usize)?;
                self.raw_message()
                    .map_err(|_| self.note_oom())
                    .ok()?
                    .get(part.raw_body_offset() as usize..part.raw_end_offset() as usize)
                    .map(|v| self.bytes_variable(v))
            }
        }
    }

    #[inline(always)]
    pub(crate) fn bytes_variable(&self, bytes: &'x [u8]) -> Variable<'x> {
        match std::str::from_utf8(bytes) {
            Ok(text) => Variable::borrowed(text),
            Err(_) => Variable::borrowed(self.alloc_str(&String::from_utf8_lossy(bytes))),
        }
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
        let mut result: SmallVec<[Variable<'x>; 4]> = SmallVec::new();
        let Some(part) = self.message.part(self.part) else {
            return Ok(None);
        };
        let raw = self.raw_message()?;
        if !header.names.is_empty() {
            let mut names: SmallVec<[&HeaderName<'static>; 2]> = SmallVec::new();
            for rec in script.recs(header.names)? {
                names.push(script.header_name(rec.c)?);
            }
            let mut headers = part
                .headers
                .iter()
                .filter(|h| names.iter().any(|n| **n == h.name));
            match header.index_hdr.cmp(&0) {
                Ordering::Greater => {
                    if let Some(h) = headers.nth((header.index_hdr - 1) as usize) {
                        self.eval_header_part(header, h, raw, &mut result);
                    }
                }
                Ordering::Less => {
                    if let Some(h) = headers
                        .rev()
                        .nth((header.index_hdr.unsigned_abs() - 1) as usize)
                    {
                        self.eval_header_part(header, h, raw, &mut result);
                    }
                }
                Ordering::Equal => {
                    for h in headers {
                        self.eval_header_part(header, h, raw, &mut result);
                    }
                }
            }
        } else {
            for h in &part.headers {
                match &header.part {
                    HeaderPartRef::Raw => {
                        if let Some(var) = raw
                            .get(h.offset_field as usize..h.offset_end as usize)
                            .map(sanitize_raw_header)
                        {
                            result.push(Variable::borrowed(self.alloc_string(var)));
                        }
                    }
                    HeaderPartRef::Text => {
                        if let HeaderValue::Text(text) = &h.value {
                            result.push(Variable::borrowed(self.alloc_string(format!(
                                "{}: {}",
                                h.name.as_str(),
                                text
                            ))));
                        } else if let HeaderValue::Text(text) = MessageStream::new(
                            raw.get(h.offset_start as usize..h.offset_end as usize)
                                .unwrap_or(b""),
                        )
                        .parse_unstructured()
                        {
                            result.push(Variable::borrowed(self.alloc_string(format!(
                                "{}: {}",
                                h.name.as_str(),
                                text
                            ))));
                        }
                    }
                    _ => {
                        self.eval_header_part(header, h, raw, &mut result);
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

    fn eval_header_part(
        &self,
        header: &HeaderVar<'x>,
        h: &Header<'x>,
        raw: &'x [u8],
        result: &mut SmallVec<[Variable<'x>; 4]>,
    ) {
        let index_part = header.index_part;
        let var = match &header.part {
            HeaderPartRef::Text => match &h.value {
                HeaderValue::Text(v) if [-1, 0, 1].contains(&index_part) => {
                    Some(Variable::borrowed(self.cow_str(v)))
                }
                HeaderValue::TextList(list) => match index_part.cmp(&0) {
                    Ordering::Greater => list
                        .get((index_part - 1) as usize)
                        .map(|v| Variable::borrowed(self.cow_str(v))),
                    Ordering::Less => list
                        .iter()
                        .rev()
                        .nth((index_part.unsigned_abs() - 1) as usize)
                        .map(|v| Variable::borrowed(self.cow_str(v))),
                    Ordering::Equal => {
                        for item in list {
                            result.push(Variable::borrowed(self.cow_str(item)));
                        }
                        return;
                    }
                },
                HeaderValue::ContentType(ct) => {
                    Some(Variable::borrowed(if let Some(st) = &ct.c_subtype {
                        self.alloc_string(format!("{}/{}", ct.c_type, st))
                    } else {
                        self.cow_str(&ct.c_type)
                    }))
                }
                HeaderValue::Address(list) => {
                    let mut list = list.iter();
                    match index_part.cmp(&0) {
                        Ordering::Greater => list
                            .nth((index_part - 1) as usize)
                            .map(|a| self.addr_to_text(a)),
                        Ordering::Less => list
                            .rev()
                            .nth((index_part.unsigned_abs() - 1) as usize)
                            .map(|a| self.addr_to_text(a)),
                        Ordering::Equal => {
                            for item in list {
                                result.push(self.addr_to_text(item));
                            }
                            return;
                        }
                    }
                }
                HeaderValue::DateTime(_) => raw
                    .get(h.offset_start as usize..h.offset_end as usize)
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .map(|s| Variable::borrowed(s.trim())),
                _ => None,
            },
            HeaderPartRef::Address(part) => match &h.value {
                HeaderValue::Address(addr) => {
                    let mut list = addr.iter();
                    match index_part.cmp(&0) {
                        Ordering::Greater => list
                            .nth((index_part - 1) as usize)
                            .and_then(|a| self.addr_part(part, a)),
                        Ordering::Less => list
                            .rev()
                            .nth((index_part.unsigned_abs() - 1) as usize)
                            .and_then(|a| self.addr_part(part, a)),
                        Ordering::Equal => {
                            for item in list {
                                result.push(self.addr_part(part, item).unwrap_or_default());
                            }
                            return;
                        }
                    }
                }
                HeaderValue::Text(_) => {
                    let addr = raw
                        .get(h.offset_start as usize..h.offset_end as usize)
                        .and_then(|bytes| match MessageStream::new(bytes).parse_address() {
                            HeaderValue::Address(addr) => addr.into(),
                            _ => None,
                        });
                    if let Some(addr) = addr {
                        let mut list = addr.iter();
                        match index_part.cmp(&0) {
                            Ordering::Greater => list
                                .nth((index_part - 1) as usize)
                                .and_then(|a| part.eval_strict(a))
                                .map(|s| Variable::borrowed(self.alloc_str(s))),
                            Ordering::Less => list
                                .rev()
                                .nth((index_part.unsigned_abs() - 1) as usize)
                                .and_then(|a| part.eval_strict(a))
                                .map(|s| Variable::borrowed(self.alloc_str(s))),
                            Ordering::Equal => {
                                for item in list {
                                    result.push(
                                        part.eval_strict(item)
                                            .map(|s| Variable::borrowed(self.alloc_str(s)))
                                            .unwrap_or_default(),
                                    );
                                }
                                return;
                            }
                        }
                    } else {
                        None
                    }
                }
                _ => None,
            },
            HeaderPartRef::Date => {
                if let HeaderValue::DateTime(dt) = &h.value {
                    Variable::from(dt.to_timestamp()).into()
                } else {
                    raw.get(h.offset_start as usize..h.offset_end as usize)
                        .and_then(|bytes| match MessageStream::new(bytes).parse_date() {
                            HeaderValue::DateTime(dt) => Variable::from(dt.to_timestamp()).into(),
                            _ => None,
                        })
                }
            }
            HeaderPartRef::Id => match &h.name {
                HeaderName::MessageId | HeaderName::ResentMessageId => match &h.value {
                    HeaderValue::Text(id) => Variable::borrowed(self.cow_str(id)).into(),
                    HeaderValue::TextList(ids) => {
                        for id in ids {
                            result.push(Variable::borrowed(self.cow_str(id)));
                        }
                        return;
                    }
                    _ => None,
                },
                HeaderName::Other(_) => {
                    match MessageStream::new(
                        raw.get(h.offset_start as usize..h.offset_end as usize)
                            .unwrap_or(b""),
                    )
                    .parse_id()
                    {
                        HeaderValue::Text(id) => Variable::borrowed(self.cow_str(&id)).into(),
                        HeaderValue::TextList(ids) => {
                            for id in ids {
                                result.push(Variable::borrowed(self.cow_str(&id)));
                            }
                            return;
                        }
                        _ => None,
                    }
                }
                _ => None,
            },
            HeaderPartRef::Raw => raw
                .get(h.offset_start as usize..h.offset_end as usize)
                .map(sanitize_raw_header)
                .map(|s| Variable::borrowed(self.alloc_string(s))),
            HeaderPartRef::RawName => (h.offset_start as usize)
                .checked_sub(1)
                .and_then(|end| raw.get(h.offset_field as usize..end))
                .map(|bytes| std::str::from_utf8(bytes).unwrap_or_default())
                .map(Variable::borrowed),
            HeaderPartRef::Exists => Variable::from(true).into(),
            HeaderPartRef::ContentType(part) => match &h.value {
                HeaderValue::ContentType(ct) => match part {
                    ContentTypeRef::Type => Variable::borrowed(self.cow_str(&ct.c_type)).into(),
                    ContentTypeRef::Subtype => ct
                        .c_subtype
                        .as_ref()
                        .map(|s| Variable::borrowed(self.cow_str(s))),
                    ContentTypeRef::Attribute(attr) => ct.attributes.as_ref().and_then(|attrs| {
                        attrs.iter().find_map(|a| {
                            if a.name.eq_ignore_ascii_case(attr) {
                                Some(Variable::borrowed(self.cow_str(&a.value)))
                            } else {
                                None
                            }
                        })
                    }),
                },
                _ => None,
            },
            HeaderPartRef::Received(part) => match &h.value {
                HeaderValue::Received(rcvd) => self.received_part(part, rcvd),
                _ => None,
            },
        };

        result.push(var.unwrap_or_default());
    }

    fn addr_to_text(&self, addr: &Addr<'x>) -> Variable<'x> {
        if let Some(name) = &addr.name {
            if let Some(address) = &addr.address {
                Variable::borrowed(self.alloc_string(format!("{name} <{address}>")))
            } else {
                Variable::borrowed(self.cow_str(name))
            }
        } else if let Some(address) = &addr.address {
            Variable::borrowed(self.alloc_string(format!("<{address}>")))
        } else {
            Variable::default()
        }
    }

    fn addr_part(&self, part: &AddressPart, addr: &Addr<'x>) -> Option<Variable<'x>> {
        match part {
            AddressPart::Name => addr
                .name
                .as_ref()
                .map(|n| Variable::borrowed(self.cow_str(n))),
            AddressPart::All => addr
                .address
                .as_ref()
                .map(|a| Variable::borrowed(self.cow_str(a))),
            _ => part
                .eval_strict(addr)
                .map(|s| Variable::borrowed(self.alloc_str(s))),
        }
    }

    pub fn received_part(&self, part: &ReceivedPart, rcvd: &Received<'x>) -> Option<Variable<'x>> {
        match part {
            ReceivedPart::From(from) => rcvd
                .from()
                .or_else(|| rcvd.helo())
                .and_then(|v| self.host_variable(from, v)),
            ReceivedPart::FromIp => rcvd
                .from_ip()
                .map(|ip| Variable::borrowed(self.alloc_string(ip.to_string()))),
            ReceivedPart::FromIpRev => rcvd
                .from_iprev()
                .map(|v| Variable::borrowed(self.alloc_str(v))),
            ReceivedPart::By(by) => rcvd.by().and_then(|v: &Host<'_>| self.host_variable(by, v)),
            ReceivedPart::For => rcvd.for_().map(|v| Variable::borrowed(self.alloc_str(v))),
            ReceivedPart::With => rcvd.with().map(|v| Variable::borrowed(v.as_str())),
            ReceivedPart::TlsVersion => rcvd.tls_version().map(|v| Variable::borrowed(v.as_str())),
            ReceivedPart::TlsCipher => rcvd
                .tls_cipher()
                .map(|v| Variable::borrowed(self.alloc_str(v))),
            ReceivedPart::Id => rcvd.id().map(|v| Variable::borrowed(self.alloc_str(v))),
            ReceivedPart::Ident => rcvd.ident().map(|v| Variable::borrowed(self.alloc_str(v))),
            ReceivedPart::Via => rcvd.via().map(|v| Variable::borrowed(self.alloc_str(v))),
            ReceivedPart::Date => rcvd.date().map(|d| Variable::from(d.to_timestamp())),
            ReceivedPart::DateRaw => rcvd
                .date()
                .map(|d| Variable::borrowed(self.alloc_string(d.to_rfc822()))),
        }
    }

    fn host_variable(&self, hostname: &ReceivedHostname, host: &Host<'x>) -> Option<Variable<'x>> {
        match (hostname, host) {
            (ReceivedHostname::Name, Host::Name(name)) => {
                Variable::borrowed(self.cow_str(name)).into()
            }
            (ReceivedHostname::Ip, Host::IpAddr(ip)) => {
                Variable::borrowed(self.alloc_string(ip.to_string())).into()
            }
            (ReceivedHostname::Any, _) => {
                Variable::borrowed(self.alloc_string(host.to_string())).into()
            }
            _ => None,
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
