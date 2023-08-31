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
    Addr, Group, Header, HeaderValue, PartType,
};

use crate::{
    compiler::{
        grammar::tests::test_plugin::Plugin, HeaderPart, HeaderVariable, MessagePart, Value,
        VariableType,
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
            Value::Expression(expr) => self
                .eval_expression(expr)
                .map(Variable::from)
                .unwrap_or(Variable::default()),
            Value::Regex(r) => Variable::StringRef(&r.expr),
        }
    }

    fn eval_header<'z: 'x>(&'z self, header: &HeaderVariable) -> Option<Variable<'x>> {
        let part = self.message.part(self.part)?;
        let mut headers = part.headers.iter().filter(|h| h.name == header.name);
        let mut result = Vec::new();
        match header.index_hdr.cmp(&0) {
            Ordering::Greater => {
                if let Some(h) = headers.nth((header.index_hdr - 1) as usize) {
                    header.eval_part(h, self.message.raw_message(), &mut result);
                }
            }
            Ordering::Less => {
                if let Some(h) = headers
                    .rev()
                    .nth((header.index_hdr.unsigned_abs() - 1) as usize)
                {
                    header.eval_part(h, self.message.raw_message(), &mut result);
                }
            }
            Ordering::Equal => {
                for h in headers {
                    header.eval_part(h, self.message.raw_message(), &mut result);
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
                HeaderValue::Address(addr) if self.include_single_part() => addr.to_text().into(),
                HeaderValue::AddressList(list)
                | HeaderValue::Group(Group {
                    addresses: list, ..
                }) => match self.index_part.cmp(&0) {
                    Ordering::Greater => list
                        .get((self.index_part - 1) as usize)
                        .map(|a| a.to_text()),
                    Ordering::Less => list
                        .iter()
                        .rev()
                        .nth((self.index_part.unsigned_abs() - 1) as usize)
                        .map(|a| a.to_text()),
                    Ordering::Equal => {
                        for item in list {
                            result.push(item.to_text());
                        }
                        return;
                    }
                },
                HeaderValue::GroupList(groups) => {
                    let mut groups = groups.iter().flat_map(|group| group.addresses.iter());
                    match self.index_part.cmp(&0) {
                        Ordering::Greater => groups
                            .nth((self.index_part - 1) as usize)
                            .map(|a| a.to_text()),
                        Ordering::Less => groups
                            .rev()
                            .nth((self.index_part.unsigned_abs() - 1) as usize)
                            .map(|a| a.to_text()),
                        Ordering::Equal => {
                            for item in groups {
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
            HeaderPart::Name | HeaderPart::Address => match &header.value {
                HeaderValue::Address(addr) if self.include_single_part() => {
                    addr.part(&self.part).map(Variable::from)
                }
                HeaderValue::AddressList(list)
                | HeaderValue::Group(Group {
                    addresses: list, ..
                }) => match self.index_part.cmp(&0) {
                    Ordering::Greater => list
                        .get((self.index_part - 1) as usize)
                        .and_then(|a| a.part(&self.part))
                        .map(Variable::from),
                    Ordering::Less => list
                        .iter()
                        .rev()
                        .nth((self.index_part.unsigned_abs() - 1) as usize)
                        .and_then(|a| a.part(&self.part))
                        .map(Variable::from),
                    Ordering::Equal => {
                        for item in list {
                            if let Some(part) = item.part(&self.part) {
                                result.push(Variable::from(part));
                            }
                        }
                        return;
                    }
                },
                HeaderValue::GroupList(groups) => {
                    let mut groups = groups.iter().flat_map(|group| group.addresses.iter());
                    match self.index_part.cmp(&0) {
                        Ordering::Greater => groups
                            .nth((self.index_part - 1) as usize)
                            .and_then(|a| a.part(&self.part))
                            .map(Variable::from),
                        Ordering::Less => groups
                            .rev()
                            .nth((self.index_part.unsigned_abs() - 1) as usize)
                            .and_then(|a| a.part(&self.part))
                            .map(Variable::from),
                        Ordering::Equal => {
                            for item in groups {
                                if let Some(part) = item.part(&self.part) {
                                    result.push(Variable::from(part));
                                }
                            }
                            return;
                        }
                    }
                }
                _ => None,
            },

            HeaderPart::Raw => raw
                .get(header.offset_start..header.offset_end)
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .map(|s| s.trim())
                .map(Variable::from),
            _ => {
                if let HeaderValue::ContentType(ct) = &header.value {
                    match &self.part {
                        HeaderPart::Type => Variable::from(ct.c_type.as_ref()).into(),
                        HeaderPart::Subtype => {
                            ct.c_subtype.as_ref().map(|s| Variable::from(s.as_ref()))
                        }
                        HeaderPart::Attribute(attr) => ct.attributes.as_ref().and_then(|attrs| {
                            attrs.iter().find_map(|(k, v)| {
                                if k.eq_ignore_ascii_case(attr) {
                                    Some(Variable::from(v.as_ref()))
                                } else {
                                    None
                                }
                            })
                        }),
                        _ => None,
                    }
                } else {
                    None
                }
            }
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

trait GetAddressPart<'x> {
    fn part<'z: 'x>(&'z self, part: &HeaderPart) -> Option<&'x str>;
    fn to_text<'z: 'x>(&'z self) -> Variable<'x>;
}

impl<'x> GetAddressPart<'x> for Addr<'x> {
    fn part<'z: 'x>(&'z self, part: &HeaderPart) -> Option<&'x str> {
        match part {
            HeaderPart::Address => self.address.as_ref().map(|s| s.as_ref()),
            HeaderPart::Name => self.name.as_ref().map(|s| s.as_ref()),
            _ => None,
        }
    }

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
