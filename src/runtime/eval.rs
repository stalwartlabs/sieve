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

use std::cmp::Ordering;

use mail_parser::{
    decoders::html::{html_to_text, text_to_html},
    parsers::MessageStream,
    Addr, Header, HeaderName, HeaderValue, Host, MimeHeaders, PartType,
};

use crate::{
    compiler::{
        grammar::tests::test_plugin::Plugin, ContentTypePart, HeaderPart, HeaderVariable,
        MessagePart, ReceivedHostname, ReceivedPart, Value, VariableType,
    },
    Context, Event, PluginArgument,
};

use super::Variable;

impl<'x> Context<'x> {
    pub(crate) fn variable<'y: 'x>(&'y self, var: &VariableType) -> Option<Variable<'x>> {
        match var {
            VariableType::Local(var_num) => self.vars_local.get(*var_num).map(|v| v.as_ref()),
            VariableType::Match(var_num) => self.vars_match.get(*var_num).map(|v| v.as_ref()),
            VariableType::Global(var_name) => {
                self.vars_global.get(var_name.as_str()).map(|v| v.as_ref())
            }
            VariableType::Environment(var_name) => self
                .vars_env
                .get(var_name.as_str())
                .or_else(|| self.runtime.environment.get(var_name.as_str()))
                .map(|v| v.as_ref()),
            VariableType::Envelope(envelope) => self.envelope.iter().find_map(|(e, v)| {
                if e == envelope {
                    Some(v.as_ref())
                } else {
                    None
                }
            }),
            VariableType::Header(header) => self.eval_header(header),
            VariableType::Part(part) => match part {
                MessagePart::TextBody(convert) => {
                    let part = self.message.parts.get(*self.message.text_body.first()?)?;
                    match &part.body {
                        PartType::Text(text) => Some(text.as_ref().into()),
                        PartType::Html(html) if *convert => {
                            Some(html_to_text(html.as_ref()).into())
                        }
                        _ => None,
                    }
                }
                MessagePart::HtmlBody(convert) => {
                    let part = self.message.parts.get(*self.message.html_body.first()?)?;
                    match &part.body {
                        PartType::Html(html) => Some(html.as_ref().into()),
                        PartType::Text(text) if *convert => {
                            Some(text_to_html(text.as_ref()).into())
                        }
                        _ => None,
                    }
                }
                MessagePart::Contents => match &self.message.parts.get(self.part)?.body {
                    PartType::Text(text) | PartType::Html(text) => {
                        Variable::from(text.as_ref()).into()
                    }
                    PartType::Binary(bin) | PartType::InlineBinary(bin) => {
                        Variable::from(String::from_utf8_lossy(bin.as_ref())).into()
                    }
                    _ => None,
                },
                MessagePart::Raw => {
                    let part = self.message.parts.get(self.part)?;
                    self.message
                        .raw_message()
                        .get(part.raw_body_offset()..part.raw_end_offset())
                        .map(|v| Variable::from(String::from_utf8_lossy(v)))
                }
                MessagePart::Name => self
                    .message
                    .parts
                    .get(self.part)?
                    .attachment_name()
                    .map(Variable::from),
                MessagePart::IsEncodingProblem => {
                    Variable::from(self.message.parts.get(self.part)?.is_encoding_problem).into()
                }
                MessagePart::IsAttachment => {
                    Variable::from(self.message.attachments.contains(&self.part)).into()
                }
            },
        }
    }

    pub(crate) fn eval_value<'z: 'y, 'y>(&'z self, string: &'y Value) -> Variable<'y> {
        match string {
            Value::Text(text) => Variable::String(text.into()),
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
                                data.push_str(&value.to_cow());
                            }
                        }
                        Value::List(_) => {
                            debug_assert!(false, "This should not have happened: {string:?}");
                        }
                        Value::Number(n) => {
                            data.push_str(&n.to_string());
                        }
                        Value::Expression(expr) => {
                            if let Some(value) = self.eval_expression(expr) {
                                data.push_str(&value.to_string());
                            }
                        }
                        Value::Regex(_) => (),
                    }
                }
                data.into()
            }
            Value::Number(n) => Variable::from(*n),
            Value::Expression(expr) => self.eval_expression(expr).unwrap_or(Variable::default()),
            Value::Regex(r) => Variable::StringRef(&r.expr),
        }
    }

    fn eval_header<'z: 'x>(&'z self, header: &HeaderVariable) -> Option<Variable<'x>> {
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
                            .get(h.offset_field..h.offset_end)
                            .map(sanitize_raw_header)
                        {
                            result.push(Variable::from(var));
                        }
                    }
                    HeaderPart::Text => {
                        if let HeaderValue::Text(text) = &h.value {
                            result.push(Variable::from(format!("{}: {}", h.name.as_str(), text)));
                        } else if let HeaderValue::Text(text) =
                            MessageStream::new(raw.get(h.offset_start..h.offset_end).unwrap_or(b""))
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

        if !result.is_empty() {
            Some(Variable::Array(result))
        } else {
            None
        }
    }

    #[inline(always)]
    pub(crate) fn eval_values<'z: 'y, 'y>(&'z self, strings: &'y [Value]) -> Vec<Variable<'y>> {
        strings.iter().map(|s| self.eval_value(s)).collect()
    }

    #[inline(always)]
    pub(crate) fn eval_values_owned(&self, strings: &[Value]) -> Vec<String> {
        strings
            .iter()
            .map(|s| self.eval_value(s).into_cow().into_owned())
            .collect()
    }

    pub(crate) fn eval_plugin_arguments(&self, plugin: &Plugin) -> Event {
        let mut arguments = Vec::with_capacity(plugin.arguments.len());
        for argument in &plugin.arguments {
            arguments.push(match argument {
                PluginArgument::Tag(tag) => PluginArgument::Tag(*tag),
                PluginArgument::Text(t) => PluginArgument::Text(self.eval_value(t).into_string()),
                PluginArgument::Number(n) => PluginArgument::Number(self.eval_value(n).to_number()),
                PluginArgument::Regex(r) => PluginArgument::Regex(r.clone()),
                PluginArgument::Array(a) => {
                    let mut arr = Vec::with_capacity(a.len());
                    for item in a {
                        arr.push(match item {
                            PluginArgument::Tag(tag) => PluginArgument::Tag(*tag),
                            PluginArgument::Text(t) => {
                                PluginArgument::Text(self.eval_value(t).into_string())
                            }
                            PluginArgument::Number(n) => {
                                PluginArgument::Number(self.eval_value(n).to_number())
                            }
                            PluginArgument::Regex(r) => PluginArgument::Regex(r.clone()),
                            PluginArgument::Variable(var) => PluginArgument::Variable(var.clone()),
                            PluginArgument::Array(_) => continue,
                        });
                    }
                    PluginArgument::Array(arr)
                }
                PluginArgument::Variable(var) => PluginArgument::Variable(var.clone()),
            });
        }

        Event::Plugin {
            id: plugin.id,
            arguments,
        }
    }
}

impl HeaderVariable {
    fn eval_part<'x>(&self, header: &'x Header<'x>, raw: &'x [u8], result: &mut Vec<Variable<'x>>) {
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
                    .get(header.offset_start..header.offset_end)
                    .and_then(|bytes| std::str::from_utf8(bytes).ok())
                    .map(|s| s.trim())
                    .map(Variable::from),
                _ => None,
            },
            HeaderPart::Address(addr) => match &header.value {
                HeaderValue::Address(list) => {
                    let mut list = list.iter();
                    match self.index_part.cmp(&0) {
                        Ordering::Greater => list
                            .nth((self.index_part - 1) as usize)
                            .and_then(|a| addr.eval(a))
                            .map(Variable::from),
                        Ordering::Less => list
                            .rev()
                            .nth((self.index_part.unsigned_abs() - 1) as usize)
                            .and_then(|a| addr.eval(a))
                            .map(Variable::from),
                        Ordering::Equal => {
                            for item in list {
                                if let Some(part) = addr.eval(item) {
                                    result.push(Variable::from(part));
                                }
                            }
                            return;
                        }
                    }
                }
                _ => None,
            },
            HeaderPart::Date => {
                if let HeaderValue::DateTime(dt) = &header.value {
                    Variable::from(dt.to_timestamp()).into()
                } else {
                    raw.get(header.offset_start..header.offset_end)
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
                        raw.get(header.offset_start..header.offset_end)
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
                .get(header.offset_start..header.offset_end)
                .map(sanitize_raw_header)
                .map(Variable::from),
            _ => match (&header.value, &self.part) {
                (HeaderValue::ContentType(ct), HeaderPart::ContentType(part)) => match part {
                    ContentTypePart::Type => Variable::from(ct.c_type.as_ref()).into(),
                    ContentTypePart::Subtype => {
                        ct.c_subtype.as_ref().map(|s| Variable::from(s.as_ref()))
                    }
                    ContentTypePart::Attribute(attr) => ct.attributes.as_ref().and_then(|attrs| {
                        attrs.iter().find_map(|(k, v)| {
                            if k.eq_ignore_ascii_case(attr) {
                                Some(Variable::from(v.as_ref()))
                            } else {
                                None
                            }
                        })
                    }),
                },
                (HeaderValue::Received(rcvd), HeaderPart::Received(part)) => match part {
                    ReceivedPart::From(from) => rcvd
                        .from()
                        .or_else(|| rcvd.helo())
                        .and_then(|v| from.to_variable(v)),
                    ReceivedPart::FromIp => rcvd.from_ip().map(|ip| Variable::from(ip.to_string())),
                    ReceivedPart::FromIpRev => rcvd.from_iprev().map(Variable::from),
                    ReceivedPart::By(by) => rcvd.by().and_then(|v: &Host<'_>| by.to_variable(v)),
                    ReceivedPart::For => rcvd.for_().map(Variable::from),
                    ReceivedPart::With => rcvd.with().map(|v| Variable::from(v.as_str())),
                    ReceivedPart::TlsVersion => {
                        rcvd.tls_version().map(|v| Variable::from(v.as_str()))
                    }
                    ReceivedPart::TlsCipher => rcvd.tls_cipher().map(Variable::from),
                    ReceivedPart::Id => rcvd.id().map(Variable::from),
                    ReceivedPart::Ident => rcvd.ident().map(Variable::from),
                    ReceivedPart::Via => rcvd.via().map(Variable::from),
                    ReceivedPart::Date => rcvd.date().map(|d| Variable::from(d.to_timestamp())),
                    ReceivedPart::DateRaw => rcvd.date().map(|d| Variable::from(d.to_rfc822())),
                },
                _ => None,
            },
        };

        if let Some(var) = var {
            result.push(var);
        }
    }

    #[inline(always)]
    fn include_single_part(&self) -> bool {
        [-1, 0, 1].contains(&self.index_part)
    }
}

trait AddrToText<'x> {
    fn to_text<'z: 'x>(&'z self) -> Variable<'x>;
}

impl<'x> AddrToText<'x> for Addr<'x> {
    fn to_text<'z: 'x>(&'z self) -> Variable<'x> {
        if let Some(name) = &self.name {
            if let Some(address) = &self.address {
                Variable::String(format!("{name} <{address}>"))
            } else {
                Variable::StringRef(name.as_ref())
            }
        } else if let Some(address) = &self.address {
            Variable::String(format!("<{address}>"))
        } else {
            Variable::StringRef("")
        }
    }
}

impl ReceivedHostname {
    fn to_variable<'x>(&self, host: &'x Host<'x>) -> Option<Variable<'x>> {
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
                if !result.is_empty() {
                    result.push(b' ');
                }
                last_is_space = false;
            }
            result.push(ch);
        }
    }

    result.into_string()
}
