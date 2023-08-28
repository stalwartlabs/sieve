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

use mail_builder::headers::{date::Date, message_id::generate_message_id_header};
use mail_parser::{decoders::quoted_printable::HEX_MAP, HeaderName, RfcHeader};

use crate::{
    compiler::grammar::actions::{
        action_notify::Notify,
        action_redirect::{ByTime, Ret},
    },
    Context, Event, Importance, Recipient,
};

use super::action_vacation::MAX_SUBJECT_LEN;

impl Notify {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        // Do not notify on Auto-Submitted messages
        for header in &ctx.message.parts[0].headers {
            if matches!(&header.name, HeaderName::Other(name) if name.eq_ignore_ascii_case("Auto-Submitted"))
                && header
                    .value
                    .as_text_ref()
                    .map_or(true, |v| !v.eq_ignore_ascii_case("no"))
            {
                return;
            }
        }

        let uri = ctx.eval_value(&self.method).into_string();
        let (scheme, params) = if let Some(parts) = parse_uri(&uri) {
            parts
        } else {
            return;
        };

        let has_fcc = self.fcc.is_some();
        let is_mailto = scheme.eq_ignore_ascii_case("mailto")
            && ctx.num_out_messages < ctx.runtime.max_out_messages;
        let mut events = Vec::with_capacity(3);

        if is_mailto || has_fcc {
            let params = if is_mailto {
                if let Some(params) = parse_mailto(params) {
                    params
                } else {
                    return;
                }
            } else {
                MailtoMessage {
                    to: Vec::new(),
                    cc: Vec::new(),
                    bcc: Vec::new(),
                    body: None,
                    headers: Vec::new(),
                }
            };
            let from = if let Some(from) = &self.from {
                let from = ctx.eval_value(from).into_cow();
                if from
                    .to_ascii_lowercase()
                    .contains(&ctx.user_address.to_ascii_lowercase())
                {
                    from
                } else {
                    ctx.user_from_field().into()
                }
            } else {
                ctx.user_from_field().into()
            };
            let notify_message = self.message.as_ref().map(|m| ctx.eval_value(m).into_cow());
            let message_len = params
                .to
                .iter()
                .chain(params.cc.iter())
                .map(|a| a.len() + 4)
                .sum::<usize>()
                + params
                    .headers
                    .iter()
                    .map(|(h, v)| h.len() + v.len() + 4)
                    .sum::<usize>()
                + params.body.as_ref().map_or(0, |b| b.len())
                + notify_message.as_ref().map_or(0, |b| b.len())
                + from.len()
                + 200;

            let mut message = Vec::with_capacity(message_len);
            message.extend_from_slice(b"From: ");
            message.extend_from_slice(from.as_bytes());
            message.extend_from_slice(b"\r\n");

            for (header, addresses) in [("To: ", &params.to), ("Cc: ", &params.cc)] {
                if !addresses.is_empty() {
                    message.extend_from_slice(header.as_bytes());
                    for (pos, address) in addresses.iter().enumerate() {
                        if pos > 0 {
                            message.extend_from_slice(b", ");
                        }
                        if !address.contains('<') {
                            message.push(b'<');
                        }
                        message.extend_from_slice(address.as_bytes());
                        if !address.contains('<') {
                            message.push(b'>');
                        }
                    }
                    message.extend_from_slice(b"\r\n");
                }
            }

            let mut has_subject = None;
            let mut has_date = false;
            let mut has_message_id = false;
            for (header, value) in &params.headers {
                match header {
                    HeaderName::Rfc(RfcHeader::Subject) => {
                        has_subject = value.into();
                        continue;
                    }
                    HeaderName::Rfc(RfcHeader::Date) => {
                        has_date = true;
                    }
                    HeaderName::Rfc(RfcHeader::MessageId) => {
                        has_message_id = true;
                    }
                    HeaderName::Rfc(RfcHeader::From) => {
                        continue;
                    }
                    _ => (),
                }
                message.extend_from_slice(header.as_str().as_bytes());
                message.extend_from_slice(b": ");
                message.extend_from_slice(value.as_bytes());
                message.extend_from_slice(b"\r\n");
            }

            if !has_date {
                message.extend_from_slice(b"Date: ");
                message.extend_from_slice(Date::now().to_rfc822().as_bytes());
                message.extend_from_slice(b"\r\n");
            }

            if !has_message_id {
                message.extend_from_slice(b"Message-ID: ");
                generate_message_id_header(&mut message, &ctx.runtime.local_hostname).unwrap();
                message.extend_from_slice(b"\r\n");
            }

            let (importance, priority) =
                self.importance
                    .as_ref()
                    .map_or(("Normal", "3 (Normal)"), |i| {
                        match ctx.eval_value(i).into_cow().as_ref() {
                            "1" => ("High", "1 (High)"),
                            "3" => ("Low", "5 (Low)"),
                            _ => ("Normal", "3 (Normal)"),
                        }
                    });
            message.extend_from_slice(b"Importance: ");
            message.extend_from_slice(importance.as_bytes());
            message.extend_from_slice(b"\r\n");

            message.extend_from_slice(b"X-Priority: ");
            message.extend_from_slice(priority.as_bytes());
            message.extend_from_slice(b"\r\n");

            message.extend_from_slice(b"Subject: ");
            let subject = if let Some(subject) = has_subject {
                subject.as_str()
            } else if let Some(subject) = &notify_message {
                subject.as_ref()
            } else if let Some(subject) = ctx.message.subject() {
                subject
            } else {
                ""
            };
            let mut iter = subject.chars().enumerate();
            let mut buf = [0; 4];
            #[allow(clippy::while_let_on_iterator)]
            while let Some((pos, char)) = iter.next() {
                if pos < MAX_SUBJECT_LEN {
                    message.extend_from_slice(char.encode_utf8(&mut buf).as_bytes());
                } else {
                    break;
                }
            }
            if iter.next().is_some() {
                message.extend_from_slice('…'.encode_utf8(&mut buf).as_bytes());
            }
            message.extend_from_slice(b"\r\n");

            message.extend_from_slice(b"Auto-Submitted: auto-notified\r\n");
            message.extend_from_slice(b"X-Sieve: yes\r\n");
            message.extend_from_slice(b"Content-type: text/plain; charset=utf-8\r\n\r\n");
            if let Some(body) = params.body {
                message.extend_from_slice(body.as_bytes());
            } else if let Some(subject) = &notify_message {
                message.extend_from_slice(subject.as_bytes());
            } else if let Some(subject) = ctx.message.subject() {
                message.extend_from_slice(subject.as_bytes());
            }

            ctx.last_message_id += 1;
            events.push(Event::CreatedMessage {
                message_id: ctx.last_message_id,
                message,
            });

            if is_mailto {
                events.push(Event::SendMessage {
                    recipient: Recipient::Group(
                        params
                            .to
                            .into_iter()
                            .chain(params.cc)
                            .chain(params.bcc)
                            .map(|addr| {
                                if let Some((addr, _)) = addr
                                    .rsplit_once('<')
                                    .and_then(|(_, addr)| addr.rsplit_once('>'))
                                {
                                    addr.to_string()
                                } else {
                                    addr
                                }
                            })
                            .collect(),
                    ),
                    notify: crate::compiler::grammar::actions::action_redirect::Notify::Never,
                    return_of_content: Ret::Default,
                    by_time: ByTime::None,
                    message_id: ctx.last_message_id,
                });
            }
        }

        if !is_mailto {
            events.push(Event::Notify {
                method: uri,
                from: self.from.as_ref().map(|f| ctx.eval_value(f).into_string()),
                importance: self.importance.as_ref().map_or(Importance::Normal, |i| {
                    match ctx.eval_value(i).into_cow().as_ref() {
                        "1" => Importance::High,
                        "3" => Importance::Low,
                        _ => Importance::Normal,
                    }
                }),
                options: ctx.eval_values_owned(&self.options),
                message: self
                    .message
                    .as_ref()
                    .map(|m| ctx.eval_value(m).into_string())
                    .or_else(|| ctx.message.subject().map(|s| s.to_string()))
                    .unwrap_or_default(),
            });
            ctx.num_out_messages += 1;
        }

        if let Some(fcc) = &self.fcc {
            // File carbon copy
            events.push(Event::FileInto {
                folder: ctx.eval_value(&fcc.mailbox).into_string(),
                flags: ctx.get_local_flags(&fcc.flags),
                mailbox_id: fcc
                    .mailbox_id
                    .as_ref()
                    .map(|m| ctx.eval_value(m).into_string()),
                special_use: fcc
                    .special_use
                    .as_ref()
                    .map(|s| ctx.eval_value(s).into_string()),
                create: fcc.create,
                message_id: ctx.last_message_id,
            });
        }
        ctx.queued_events = events.into_iter();
    }
}

pub fn validate_from(addr: &str) -> bool {
    let mut has_at = false;
    let mut has_dot = false;
    let mut in_quote = false;
    let mut in_angle = false;
    let mut last_ch = 0;

    for &ch in addr.as_bytes().iter() {
        match ch {
            b'\"' => {
                if last_ch != b'\\' {
                    in_quote = !in_quote;
                }
            }
            b'<' if !in_quote => {
                if !in_angle {
                    in_angle = true;
                    has_at = false;
                    has_dot = false;
                } else {
                    return false;
                }
            }
            b'>' if !in_quote => {
                if in_angle {
                    in_angle = false;
                } else {
                    return false;
                }
            }
            b'@' if !in_quote => {
                if !has_at && last_ch.is_ascii_alphanumeric() {
                    has_at = true;
                } else {
                    return false;
                }
            }
            b'.' if !in_quote && has_at => {
                has_dot = true;
            }
            _ => (),
        }
        last_ch = ch;
    }

    has_dot && has_at && !in_angle
}

pub fn validate_uri(uri: &str) -> Option<&str> {
    let (scheme, uri) = parse_uri(uri)?;
    if scheme.eq_ignore_ascii_case("mailto") {
        parse_mailto(uri)?;
        scheme.into()
    } else if ["xmpp", "tel", "http", "https"].contains(&scheme) {
        scheme.into()
    } else {
        None
    }
}

pub(crate) fn parse_uri(uri: &str) -> Option<(&str, &str)> {
    let (scheme, uri) = uri.split_once(':')?;

    if !uri.is_empty() {
        Some((scheme, uri))
    } else {
        None
    }
}

pub enum Mailto {
    Header(RfcHeader),
    Body,
    Other(String),
}

enum State {
    Address((RfcHeader, bool)),
    ParamName,
    ParamValue(Mailto),
}

#[derive(Default)]
struct MailtoMessage {
    to: Vec<String>,
    cc: Vec<String>,
    bcc: Vec<String>,
    body: Option<String>,
    headers: Vec<(HeaderName<'static>, String)>,
}

fn parse_mailto(uri: &str) -> Option<MailtoMessage> {
    let mut params = MailtoMessage::default();

    let mut state = State::Address((RfcHeader::To, false));
    let mut buf = Vec::new();
    let uri_ = uri.as_bytes();
    let mut iter = uri_.iter();
    let mut has_addresses = false;

    while let Some(&ch) = iter.next() {
        match ch {
            b'%' => {
                let hex1 = HEX_MAP[*iter.next()? as usize];
                let hex2 = HEX_MAP[*iter.next()? as usize];
                if hex1 != -1 && hex2 != -1 {
                    let ch = ((hex1 as u8) << 4) | hex2 as u8;

                    match &state {
                        State::Address((header, has_at)) => match ch {
                            b',' => {
                                if *has_at {
                                    insert_address(
                                        &mut params,
                                        *header,
                                        String::from_utf8(std::mem::take(&mut buf)).ok()?,
                                    );
                                    has_addresses = true;
                                    state = State::Address((*header, false));
                                } else {
                                    return None;
                                }
                            }
                            b'@' => {
                                if !*has_at {
                                    state = State::Address((*header, true));
                                    buf.push(ch);
                                } else {
                                    return None;
                                }
                            }
                            _ => {
                                buf.push(ch);
                            }
                        },
                        _ => buf.push(ch),
                    }
                } else {
                    return None;
                }
            }
            b',' => match &state {
                State::Address((header, true)) => {
                    insert_address(
                        &mut params,
                        *header,
                        String::from_utf8(std::mem::take(&mut buf)).ok()?,
                    );
                    state = State::Address((*header, false));
                    has_addresses = true;
                }
                State::ParamValue(_) => buf.push(ch),
                _ => return None,
            },
            b'?' => match &state {
                State::Address((header, has_at)) if *has_at || buf.is_empty() => {
                    if !buf.is_empty() {
                        insert_address(
                            &mut params,
                            *header,
                            String::from_utf8(std::mem::take(&mut buf)).ok()?,
                        );
                        has_addresses = true;
                    }
                    state = State::ParamName;
                }
                State::ParamValue(_) => buf.push(ch),
                _ => return None,
            },
            b'@' => match &state {
                State::Address((header, false)) if !buf.is_empty() => {
                    buf.push(ch);
                    state = State::Address((*header, true));
                }
                State::ParamName | State::ParamValue(_) => buf.push(ch),
                _ => return None,
            },
            b'=' => match &state {
                State::ParamName if !buf.is_empty() => {
                    let param = String::from_utf8(std::mem::take(&mut buf)).ok()?;
                    state = if let Some(HeaderName::Rfc(header)) = HeaderName::parse(&param) {
                        if matches!(header, RfcHeader::To | RfcHeader::Cc | RfcHeader::Bcc) {
                            State::Address((header, false))
                        } else {
                            State::ParamValue(Mailto::Header(header))
                        }
                    } else if param.eq_ignore_ascii_case("body") {
                        State::ParamValue(Mailto::Body)
                    } else {
                        State::ParamValue(Mailto::Other(param))
                    };
                }
                State::ParamValue(_) => buf.push(ch),
                _ => return None,
            },
            b'&' => match state {
                State::Address((header, true)) => {
                    if !buf.is_empty() {
                        insert_address(
                            &mut params,
                            header,
                            String::from_utf8(std::mem::take(&mut buf)).ok()?,
                        );
                    }
                    state = State::ParamName;
                }
                State::ParamValue(param) => {
                    if !buf.is_empty() {
                        let value = String::from_utf8(std::mem::take(&mut buf)).ok()?;
                        match param {
                            Mailto::Header(header) => {
                                params.headers.push((HeaderName::Rfc(header), value))
                            }
                            Mailto::Body => params.body = value.into(),
                            Mailto::Other(header) => params
                                .headers
                                .push((HeaderName::Other(header.into()), value)),
                        }
                    }
                    state = State::ParamName;
                }
                _ => return None,
            },
            _ => match &state {
                State::ParamName => {
                    if ch.is_ascii_alphanumeric() || [b'-', b'_'].contains(&ch) {
                        buf.push(ch);
                    } else {
                        return None;
                    }
                }
                _ => {
                    if !ch.is_ascii_whitespace() {
                        buf.push(ch);
                    }
                }
            },
        }
    }

    if !buf.is_empty() {
        let value = String::from_utf8(std::mem::take(&mut buf)).ok()?;
        match state {
            State::Address((header, true)) => {
                insert_address(&mut params, header, value);
                has_addresses = true;
            }
            State::ParamName => {
                params
                    .headers
                    .push((HeaderName::Other(value.into()), String::new()));
            }
            State::ParamValue(param) => match param {
                Mailto::Header(header) => params.headers.push((HeaderName::Rfc(header), value)),
                Mailto::Body => params.body = value.into(),
                Mailto::Other(header) => params
                    .headers
                    .push((HeaderName::Other(header.into()), value)),
            },
            _ => return None,
        }
    }

    if has_addresses {
        Some(params)
    } else {
        None
    }
}

#[inline(always)]
fn insert_address(params: &mut MailtoMessage, name: RfcHeader, value: String) {
    if !params
        .to
        .iter()
        .chain(params.cc.iter())
        .chain(params.bcc.iter())
        .any(|v| v.eq_ignore_ascii_case(&value))
    {
        match name {
            RfcHeader::To => {
                params.to.push(value);
            }
            RfcHeader::Cc => {
                params.cc.push(value);
            }
            RfcHeader::Bcc => {
                params.bcc.push(value);
            }
            _ => (),
        }
    }
}
