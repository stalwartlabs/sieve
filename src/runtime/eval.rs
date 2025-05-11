/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::cmp::Ordering;

use mail_parser::{
    decoders::html::{html_to_text, text_to_html},
    parsers::MessageStream,
    Addr, Header, HeaderName, HeaderValue, Host, PartType, Received,
};

use crate::{
    compiler::{
        ContentTypePart, HeaderPart, HeaderVariable, MessagePart, ReceivedHostname, ReceivedPart,
        Value, VariableType,
    },
    Context,
};

use super::Variable;

impl<'x> Context<'x> {
    pub(crate) fn variable<'y: 'x>(&'y self, var: &VariableType) -> Option<Variable> {
        match var {
            VariableType::Local(var_num) => self.vars_local.get(*var_num).cloned(),
            VariableType::Match(var_num) => self.vars_match.get(*var_num).cloned(),
            VariableType::Global(var_name) => self.vars_global.get(var_name.as_str()).cloned(),
            VariableType::Environment(var_name) => self
                .vars_env
                .get(var_name.as_str())
                .or_else(|| self.runtime.environment.get(var_name.as_str()))
                .cloned(),
            VariableType::Envelope(envelope) => {
                self.envelope.iter().find_map(
                    |(e, v)| {
                        if e == envelope {
                            Some(v.clone())
                        } else {
                            None
                        }
                    },
                )
            }
            VariableType::Header(header) => self.eval_header(header),
            VariableType::Part(part) => match part {
                MessagePart::TextBody(convert) => {
                    let part = self
                        .message
                        .parts
                        .get(*self.message.text_body.first()? as usize)?;
                    match &part.body {
                        PartType::Text(text) => Some(text.as_ref().into()),
                        PartType::Html(html) if *convert => {
                            Some(html_to_text(html.as_ref()).into())
                        }
                        _ => None,
                    }
                }
                MessagePart::HtmlBody(convert) => {
                    let part = self
                        .message
                        .parts
                        .get(*self.message.html_body.first()? as usize)?;
                    match &part.body {
                        PartType::Html(html) => Some(html.as_ref().into()),
                        PartType::Text(text) if *convert => {
                            Some(text_to_html(text.as_ref()).into())
                        }
                        _ => None,
                    }
                }
                MessagePart::Contents => match &self.message.parts.get(self.part as usize)?.body {
                    PartType::Text(text) | PartType::Html(text) => {
                        Variable::from(text.as_ref()).into()
                    }
                    PartType::Binary(bin) | PartType::InlineBinary(bin) => {
                        Variable::from(String::from_utf8_lossy(bin.as_ref())).into()
                    }
                    _ => None,
                },
                MessagePart::Raw => {
                    let part = self.message.parts.get(self.part as usize)?;
                    self.message
                        .raw_message()
                        .get(part.raw_body_offset() as usize..part.raw_end_offset() as usize)
                        .map(|v| Variable::from(String::from_utf8_lossy(v)))
                }
            },
        }
    }

    pub(crate) fn eval_value(&self, string: &Value) -> Variable {
        match string {
            Value::Text(text) => Variable::String(text.clone()),
            Value::Variable(var) => self.variable(var).unwrap_or_default(),
            Value::List(list) => {
                let mut data = String::new();
                for item in list {
                    match item {
                        Value::Text(string) => {
                            data.push_str(string);
                        }
                        Value::Variable(var) => {
                            if let Some(value) = self.variable(var) {
                                data.push_str(&value.to_string());
                            }
                        }
                        Value::List(_) => {
                            debug_assert!(false, "This should not have happened: {string:?}");
                        }
                        Value::Number(n) => {
                            data.push_str(&n.to_string());
                        }
                        Value::Regex(_) => (),
                    }
                }
                data.into()
            }
            Value::Number(n) => Variable::from(*n),
            Value::Regex(r) => Variable::String(r.expr.clone().into()),
        }
    }

    fn eval_header<'z: 'x>(&'z self, header: &HeaderVariable) -> Option<Variable> {
        let mut result = Vec::new();
        let part = self.message.part(self.part)?;
        let raw = self.message.raw_message();
        if !header.name.is_empty() {
            let mut headers = part
                .headers
                .iter()
                .filter(|h| header.name.contains(&h.name));
            match header.index_hdr.cmp(&0) {
                Ordering::Greater => {
                    if let Some(h) = headers.nth((header.index_hdr - 1) as usize) {
                        header.eval_part(h, raw, &mut result);
                    }
                }
                Ordering::Less => {
                    if let Some(h) = headers
                        .rev()
                        .nth((header.index_hdr.unsigned_abs() - 1) as usize)
                    {
                        header.eval_part(h, raw, &mut result);
                    }
                }
                Ordering::Equal => {
                    for h in headers {
                        header.eval_part(h, raw, &mut result);
                    }
                }
            }
        } else {
            for h in &part.headers {
                match &header.part {
                    HeaderPart::Raw => {
                        if let Some(var) = raw
                            .get(h.offset_field as usize..h.offset_end as usize)
                            .map(sanitize_raw_header)
                        {
                            result.push(Variable::from(var));
                        }
                    }
                    HeaderPart::Text => {
                        if let HeaderValue::Text(text) = &h.value {
                            result.push(Variable::from(format!("{}: {}", h.name.as_str(), text)));
                        } else if let HeaderValue::Text(text) = MessageStream::new(
                            raw.get(h.offset_start as usize..h.offset_end as usize)
                                .unwrap_or(b""),
                        )
                        .parse_unstructured()
                        {
                            result.push(Variable::from(format!("{}: {}", h.name.as_str(), text)));
                        }
                    }
                    _ => {
                        header.eval_part(h, raw, &mut result);
                    }
                }
            }
        }

        match result.len() {
            1 if header.index_hdr != 0 && header.index_part != 0 => result.pop(),
            0 => None,
            _ => Some(Variable::Array(result.into())),
        }
    }

    #[inline(always)]
    pub(crate) fn eval_values<'z: 'y, 'y>(&'z self, strings: &'y [Value]) -> Vec<Variable> {
        strings.iter().map(|s| self.eval_value(s)).collect()
    }

    #[inline(always)]
    pub(crate) fn eval_values_owned(&self, strings: &[Value]) -> Vec<String> {
        strings
            .iter()
            .map(|s| self.eval_value(s).to_string().into_owned())
            .collect()
    }
}

impl HeaderVariable {
    fn eval_part<'x>(&self, header: &'x Header<'x>, raw: &'x [u8], result: &mut Vec<Variable>) {
        let var = match &self.part {
            HeaderPart::Text => match &header.value {
                HeaderValue::Text(v) if self.include_single_part() => {
                    Some(Variable::from(v.as_ref()))
                }
                HeaderValue::TextList(list) => match self.index_part.cmp(&0) {
                    Ordering::Greater => list
                        .get((self.index_part - 1) as usize)
                        .map(|v| Variable::from(v.as_ref())),
                    Ordering::Less => list
                        .iter()
                        .rev()
                        .nth((self.index_part.unsigned_abs() - 1) as usize)
                        .map(|v| Variable::from(v.as_ref())),
                    Ordering::Equal => {
                        for item in list {
                            result.push(Variable::from(item.as_ref()));
                        }
                        return;
                    }
                },
                HeaderValue::ContentType(ct) => if let Some(st) = &ct.c_subtype {
                    Variable::from(format!("{}/{}", ct.c_type, st))
                } else {
                    Variable::from(ct.c_type.as_ref())
                }
                .into(),
                HeaderValue::Address(list) => {
                    let mut list = list.iter();
                    match self.index_part.cmp(&0) {
                        Ordering::Greater => list
                            .nth((self.index_part - 1) as usize)
                            .map(|a| a.to_text()),
                        Ordering::Less => list
                            .rev()
                            .nth((self.index_part.unsigned_abs() - 1) as usize)
                            .map(|a| a.to_text()),
                        Ordering::Equal => {
                            for item in list {
                                result.push(item.to_text());
                            }
                            return;
                        }
                    }
                }
                HeaderValue::DateTime(_) => raw
                    .get(header.offset_start as usize..header.offset_end as usize)
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .map(|s| s.trim())
                    .map(Variable::from),
                _ => None,
            },
            HeaderPart::Address(part) => match &header.value {
                HeaderValue::Address(addr) => {
                    let mut list = addr.iter();
                    match self.index_part.cmp(&0) {
                        Ordering::Greater => list
                            .nth((self.index_part - 1) as usize)
                            .and_then(|a| part.eval_strict(a))
                            .map(Variable::from),
                        Ordering::Less => list
                            .rev()
                            .nth((self.index_part.unsigned_abs() - 1) as usize)
                            .and_then(|a| part.eval_strict(a))
                            .map(Variable::from),
                        Ordering::Equal => {
                            for item in list {
                                result.push(
                                    part.eval_strict(item)
                                        .map(Variable::from)
                                        .unwrap_or_default(),
                                );
                            }
                            return;
                        }
                    }
                }
                HeaderValue::Text(_) => {
                    let addr = raw
                        .get(header.offset_start as usize..header.offset_end as usize)
                        .and_then(|bytes| match MessageStream::new(bytes).parse_address() {
                            HeaderValue::Address(addr) => addr.into(),
                            _ => None,
                        });
                    if let Some(addr) = addr {
                        let mut list = addr.iter();
                        match self.index_part.cmp(&0) {
                            Ordering::Greater => list
                                .nth((self.index_part - 1) as usize)
                                .and_then(|a| part.eval_strict(a))
                                .map(|s| Variable::String(s.to_string().into())),
                            Ordering::Less => list
                                .rev()
                                .nth((self.index_part.unsigned_abs() - 1) as usize)
                                .and_then(|a| part.eval_strict(a))
                                .map(|s| Variable::String(s.to_string().into())),
                            Ordering::Equal => {
                                for item in list {
                                    result.push(
                                        part.eval_strict(item)
                                            .map(|s| Variable::String(s.to_string().into()))
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
            HeaderPart::Date => {
                if let HeaderValue::DateTime(dt) = &header.value {
                    Variable::from(dt.to_timestamp()).into()
                } else {
                    raw.get(header.offset_start as usize..header.offset_end as usize)
                        .and_then(|bytes| match MessageStream::new(bytes).parse_date() {
                            HeaderValue::DateTime(dt) => Variable::from(dt.to_timestamp()).into(),
                            _ => None,
                        })
                }
            }
            HeaderPart::Id => match &header.name {
                HeaderName::MessageId | HeaderName::ResentMessageId => match &header.value {
                    HeaderValue::Text(id) => Variable::from(id.as_ref()).into(),
                    HeaderValue::TextList(ids) => {
                        for id in ids {
                            result.push(Variable::from(id.as_ref()));
                        }
                        return;
                    }
                    _ => None,
                },
                HeaderName::Other(_) => {
                    match MessageStream::new(
                        raw.get(header.offset_start as usize..header.offset_end as usize)
                            .unwrap_or(b""),
                    )
                    .parse_id()
                    {
                        HeaderValue::Text(id) => Variable::from(id).into(),
                        HeaderValue::TextList(ids) => {
                            for id in ids {
                                result.push(Variable::from(id));
                            }
                            return;
                        }
                        _ => None,
                    }
                }
                _ => None,
            },

            HeaderPart::Raw => raw
                .get(header.offset_start as usize..header.offset_end as usize)
                .map(sanitize_raw_header)
                .map(Variable::from),
            HeaderPart::RawName => raw
                .get(header.offset_field as usize..header.offset_start as usize - 1)
                .map(|bytes| std::str::from_utf8(bytes).unwrap_or_default())
                .map(Variable::from),
            HeaderPart::Exists => Variable::from(true).into(),
            _ => match (&header.value, &self.part) {
                (HeaderValue::ContentType(ct), HeaderPart::ContentType(part)) => match part {
                    ContentTypePart::Type => Variable::from(ct.c_type.as_ref()).into(),
                    ContentTypePart::Subtype => {
                        ct.c_subtype.as_ref().map(|s| Variable::from(s.as_ref()))
                    }
                    ContentTypePart::Attribute(attr) => ct.attributes.as_ref().and_then(|attrs| {
                        attrs.iter().find_map(|a| {
                            if a.name.eq_ignore_ascii_case(attr) {
                                Some(Variable::from(a.value.as_ref()))
                            } else {
                                None
                            }
                        })
                    }),
                },
                (HeaderValue::Received(rcvd), HeaderPart::Received(part)) => part.eval(rcvd),
                _ => None,
            },
        };

        result.push(var.unwrap_or_default());
    }

    #[inline(always)]
    fn include_single_part(&self) -> bool {
        [-1, 0, 1].contains(&self.index_part)
    }
}

impl ReceivedPart {
    pub fn eval<'x>(&self, rcvd: &'x Received<'x>) -> Option<Variable> {
        match self {
            ReceivedPart::From(from) => rcvd
                .from()
                .or_else(|| rcvd.helo())
                .and_then(|v| from.to_variable(v)),
            ReceivedPart::FromIp => rcvd.from_ip().map(|ip| Variable::from(ip.to_string())),
            ReceivedPart::FromIpRev => rcvd.from_iprev().map(Variable::from),
            ReceivedPart::By(by) => rcvd.by().and_then(|v: &Host<'_>| by.to_variable(v)),
            ReceivedPart::For => rcvd.for_().map(Variable::from),
            ReceivedPart::With => rcvd.with().map(|v| Variable::from(v.as_str())),
            ReceivedPart::TlsVersion => rcvd.tls_version().map(|v| Variable::from(v.as_str())),
            ReceivedPart::TlsCipher => rcvd.tls_cipher().map(Variable::from),
            ReceivedPart::Id => rcvd.id().map(Variable::from),
            ReceivedPart::Ident => rcvd.ident().map(Variable::from),
            ReceivedPart::Via => rcvd.via().map(Variable::from),
            ReceivedPart::Date => rcvd.date().map(|d| Variable::from(d.to_timestamp())),
            ReceivedPart::DateRaw => rcvd.date().map(|d| Variable::from(d.to_rfc822())),
        }
    }
}

trait AddrToText<'x> {
    fn to_text<'z: 'x>(&'z self) -> Variable;
}

impl<'x> AddrToText<'x> for Addr<'x> {
    fn to_text<'z: 'x>(&'z self) -> Variable {
        if let Some(name) = &self.name {
            if let Some(address) = &self.address {
                Variable::String(format!("{name} <{address}>").into())
            } else {
                Variable::String(name.to_string().into())
            }
        } else if let Some(address) = &self.address {
            Variable::String(format!("<{address}>").into())
        } else {
            Variable::default()
        }
    }
}

impl ReceivedHostname {
    fn to_variable<'x>(&self, host: &'x Host<'x>) -> Option<Variable> {
        match (self, host) {
            (ReceivedHostname::Name, Host::Name(name)) => Variable::from(name.as_ref()).into(),
            (ReceivedHostname::Ip, Host::IpAddr(ip)) => Variable::from(ip.to_string()).into(),
            (ReceivedHostname::Any, _) => Variable::from(host.to_string()).into(),
            _ => None,
        }
    }
}

pub(crate) trait IntoString: Sized {
    fn into_string(self) -> String;
}

pub(crate) trait ToString: Sized {
    fn to_string(&self) -> String;
}

impl IntoString for Vec<u8> {
    fn into_string(self) -> String {
        String::from_utf8(self)
            .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
    }
}

fn sanitize_raw_header(bytes: &[u8]) -> String {
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
