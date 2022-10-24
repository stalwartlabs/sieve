/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
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
use mail_parser::{Addr, HeaderName, HeaderValue, RfcHeader};

use crate::{
    compiler::grammar::{
        actions::{
            action_redirect::{ByTime, Notify, Ret},
            action_vacation::{Period, TestVacation, Vacation},
        },
        AddressPart,
    },
    runtime::{tests::TestResult, RuntimeError},
    Context, Envelope, Event, Expiry, Recipient,
};

pub(crate) const MAX_SUBJECT_LEN: usize = 256;

impl TestVacation {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut from = String::new();
        let mut user_addresses = Vec::new();

        for (name, value) in &ctx.envelope {
            if !value.is_empty() {
                match name {
                    Envelope::From => {
                        from = value.to_ascii_lowercase();
                    }
                    Envelope::To => {
                        if !ctx.runtime.vacation_use_orig_rcpt {
                            user_addresses.push(value.as_ref().into());
                        }
                    }
                    Envelope::Orcpt => {
                        if ctx.runtime.vacation_use_orig_rcpt {
                            user_addresses.push(value.as_ref().into());
                        }
                    }
                    _ => (),
                }
            }
        }

        // Add user specified addresses
        for address in &self.addresses {
            let address = ctx.eval_string(address);
            if !address.is_empty() {
                user_addresses.push(address);
            }
        }
        if !ctx.user_address.is_empty() {
            user_addresses.push(ctx.user_address.as_ref().into());
        }

        // Do not reply to own address
        if from.is_empty()
            || user_addresses.is_empty()
            || from.starts_with("mailer-daemon")
            || from.starts_with("owner-")
            || from.contains("-request@")
            || user_addresses.iter().any(|a| a.eq_ignore_ascii_case(&from))
        {
            return TestResult::Bool(false);
        }

        // Check headers
        let mut found_rcpt = false;
        let mut received_count = 0;
        for header in &ctx.message.parts[0].headers {
            match &header.name {
                HeaderName::Rfc(header_name) => match header_name {
                    RfcHeader::To
                    | RfcHeader::Cc
                    | RfcHeader::Bcc
                    | RfcHeader::ResentTo
                    | RfcHeader::ResentBcc
                    | RfcHeader::ResentCc
                        if !found_rcpt =>
                    {
                        found_rcpt = ctx.find_addresses(header, &AddressPart::All, |addr| {
                            user_addresses.iter().any(|a| a.eq_ignore_ascii_case(addr))
                        });
                    }
                    RfcHeader::ListArchive
                    | RfcHeader::ListHelp
                    | RfcHeader::ListId
                    | RfcHeader::ListOwner
                    | RfcHeader::ListPost
                    | RfcHeader::ListSubscribe
                    | RfcHeader::ListUnsubscribe => {
                        // Do not send vacation responses to lists
                        return TestResult::Bool(false);
                    }
                    RfcHeader::Received => {
                        received_count += 1;
                    }
                    _ => (),
                },
                HeaderName::Other(header_name) => {
                    if header_name.eq_ignore_ascii_case("Auto-Submitted") {
                        if header
                            .value
                            .as_text_ref()
                            .map_or(true, |v| !v.eq_ignore_ascii_case("no"))
                        {
                            return TestResult::Bool(false);
                        }
                    } else if header_name.eq_ignore_ascii_case("X-Auto-Response-Suppress") {
                        if header.value.as_text_ref().map_or(false, |v| {
                            v.to_ascii_lowercase()
                                .split(',')
                                .any(|v| ["all", "oof"].contains(&v.trim()))
                        }) {
                            return TestResult::Bool(false);
                        }
                    } else if header_name.eq_ignore_ascii_case("Precedence")
                        && header
                            .value
                            .as_text_ref()
                            .map_or(false, |v| v.eq_ignore_ascii_case("bulk"))
                    {
                        return TestResult::Bool(false);
                    }
                }
            }
        }

        // No user address found in header or possible loop
        if found_rcpt && received_count <= ctx.runtime.max_received_headers {
            TestResult::Event {
                event: Event::DuplicateId {
                    id: if let Some(handle) = &self.handle {
                        format!("_v{}{}", from, ctx.eval_string(handle))
                    } else {
                        format!("_v{}{}", from, ctx.eval_string(&self.reason))
                    },
                    expiry: match &self.period {
                        Period::Days(days) => Expiry::Seconds(days * 86400),
                        Period::Seconds(seconds) => Expiry::Seconds(*seconds),
                        Period::Default => Expiry::None,
                    },
                },
                is_not: true,
            }
        } else {
            TestResult::Bool(false)
        }
    }
}

impl Vacation {
    pub(crate) fn exec(&self, ctx: &mut Context) -> Result<(), RuntimeError> {
        let mut vacation_to = "";

        for (name, value) in &ctx.envelope {
            if !value.is_empty() && name == &Envelope::From {
                vacation_to = value.as_ref();
                break;
            }
        }

        // Check headers
        let mut vacation_subject = if let Some(subject) = &self.subject {
            ctx.eval_string(subject)
        } else {
            "".into()
        };

        // Check headers
        let mut message_id = None;
        let mut vacation_to_full = None;
        let mut references = None;
        for header in &ctx.message.parts[0].headers {
            if let HeaderName::Rfc(header_name) = &header.name {
                match header_name {
                    RfcHeader::Subject if vacation_subject.is_empty() => {
                        if let Some(subject) = header.value.as_text_ref() {
                            let mut vacation_subject_ = String::with_capacity(MAX_SUBJECT_LEN);
                            let mut iter = ctx
                                .runtime
                                .vacation_subject_prefix
                                .chars()
                                .chain(subject.chars())
                                .enumerate();

                            #[allow(clippy::while_let_on_iterator)]
                            while let Some((pos, char)) = iter.next() {
                                if pos < MAX_SUBJECT_LEN {
                                    vacation_subject_.push(char);
                                } else {
                                    break;
                                }
                            }
                            if iter.next().is_some() {
                                vacation_subject_.push('…');
                            }
                            vacation_subject = vacation_subject_.into();
                        }
                    }
                    RfcHeader::MessageId => {
                        message_id = header.value.as_text_ref();
                    }
                    RfcHeader::References => {
                        if header.offset_start > 0 {
                            references = (&ctx.message.raw_message
                                [header.offset_start..header.offset_end])
                                .into();
                        }
                    }
                    RfcHeader::From | RfcHeader::Sender => {
                        if matches!(&header.value, HeaderValue::Address(Addr { address: Some(address), ..}) if address.eq_ignore_ascii_case(vacation_to))
                            && header.offset_start > 0
                        {
                            vacation_to_full = (&ctx.message.raw_message
                                [header.offset_start..header.offset_end])
                                .into();
                        }
                    }
                    _ => (),
                }
            }
        }

        // Build message
        let vacation_from = if let Some(from) = &self.from {
            ctx.eval_string(from)
        } else if !ctx.user_address.is_empty() {
            ctx.user_from_field().into()
        } else if let Some(addr) =
            ctx.envelope
                .iter()
                .find_map(|(n, v)| if n == &Envelope::To { Some(v) } else { None })
        {
            addr.as_ref().into()
        } else {
            "".into()
        };
        if vacation_subject.is_empty() {
            vacation_subject = ctx.runtime.vacation_default_subject.as_ref().into();
        }
        let vacation_body = ctx.eval_string(&self.reason);
        let message_len = vacation_body.len()
            + vacation_from.len()
            + vacation_to_full
                .as_ref()
                .map_or(vacation_to.len(), |t| t.len())
            + vacation_subject.len()
            + message_id.as_ref().map_or(0, |m| m.len() * 2)
            + references.as_ref().map_or(0, |m| m.len())
            + 160;

        if message_len > ctx.runtime.max_memory {
            return Err(RuntimeError::OutOfMemory);
        }

        let mut message = Vec::with_capacity(message_len);
        write_header(&mut message, "From: ", vacation_from.as_ref());
        if let Some(vacation_to_full) = vacation_to_full {
            message.extend_from_slice(b"To:");
            message.extend_from_slice(vacation_to_full);
        } else {
            write_header(&mut message, "To: ", vacation_to);
        }
        write_header(&mut message, "Subject: ", vacation_subject.as_ref());
        if let Some(message_id) = message_id {
            message.extend_from_slice(b"In-Reply-To: <");
            message.extend_from_slice(message_id.as_bytes());
            message.extend_from_slice(b">\r\n");

            message.extend_from_slice(b"References: <");
            message.extend_from_slice(message_id.as_bytes());
            if let Some(references) = references {
                message.extend_from_slice(b"> ");
                message.extend_from_slice(references);
            } else {
                message.extend_from_slice(b">\r\n");
            }
        }
        message.extend_from_slice(b"Date: ");
        message.extend_from_slice(Date::now().to_rfc822().as_bytes());
        message.extend_from_slice(b"\r\n");

        message.extend_from_slice(b"Message-ID: ");
        generate_message_id_header(&mut message).unwrap();
        message.extend_from_slice(b"\r\n");

        write_header(&mut message, "Auto-Submitted: ", "auto-replied");
        if !self.mime {
            message.extend_from_slice(b"Content-type: text/plain; charset=utf-8\r\n\r\n");
        }
        message.extend_from_slice(vacation_body.as_bytes());

        // Add action
        let mut events = Vec::with_capacity(3);
        ctx.last_message_id += 1;
        events.push(Event::CreatedMessage {
            message_id: ctx.last_message_id,
            message,
        });
        events.push(Event::SendMessage {
            recipient: Recipient::Address(vacation_to.to_string()),
            notify: Notify::Never,
            return_of_content: Ret::Default,
            by_time: ByTime::None,
            message_id: ctx.last_message_id,
        });

        // File carbon copy
        if let Some(fcc) = &self.fcc {
            events.push(Event::FileInto {
                folder: ctx.eval_string(&fcc.mailbox).into_owned(),
                flags: ctx.get_local_flags(&fcc.flags),
                mailbox_id: fcc
                    .mailbox_id
                    .as_ref()
                    .map(|m| ctx.eval_string(m).into_owned()),
                special_use: fcc
                    .special_use
                    .as_ref()
                    .map(|s| ctx.eval_string(s).into_owned()),
                create: fcc.create,
                message_id: ctx.last_message_id,
            });
        }
        ctx.queued_events = events.into_iter();

        Ok(())
    }
}

fn write_header(buf: &mut Vec<u8>, name: &str, value: &str) {
    buf.extend_from_slice(name.as_bytes());
    buf.extend_from_slice(value.as_bytes());
    buf.extend_from_slice(b"\r\n");
}
