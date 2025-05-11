/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::borrow::Cow;

use mail_builder::headers::{date::Date, message_id::generate_message_id_header};
use mail_parser::{HeaderName, HeaderValue};

use crate::{
    compiler::grammar::{
        actions::{
            action_redirect::{ByTime, Notify, Ret},
            action_vacation::{Period, TestVacation, Vacation},
        },
        AddressPart,
    },
    runtime::tests::TestResult,
    Context, Envelope, Event, Recipient,
};

pub(crate) const MAX_SUBJECT_LEN: usize = 256;

impl TestVacation {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut from = String::new();
        let mut user_addresses = Vec::new();

        if ctx.num_out_messages >= ctx.runtime.max_out_messages {
            return TestResult::Bool(false);
        }

        for (name, value) in &ctx.envelope {
            if !value.is_empty() {
                match name {
                    Envelope::From => {
                        from = value.to_string().to_ascii_lowercase();
                    }
                    Envelope::To => {
                        if !ctx.runtime.vacation_use_orig_rcpt {
                            user_addresses.push(value.to_string());
                        }
                    }
                    Envelope::Orcpt => {
                        if ctx.runtime.vacation_use_orig_rcpt {
                            user_addresses.push(value.to_string());
                        }
                    }
                    _ => (),
                }
            }
        }

        // Add user specified addresses
        for address in &self.addresses {
            let address = ctx.eval_value(address).to_string().into_owned();
            if !address.is_empty() {
                user_addresses.push(address.into());
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
                HeaderName::To
                | HeaderName::Cc
                | HeaderName::Bcc
                | HeaderName::ResentTo
                | HeaderName::ResentBcc
                | HeaderName::ResentCc
                    if !found_rcpt =>
                {
                    found_rcpt = ctx.find_addresses(header, &AddressPart::All, |addr| {
                        user_addresses.iter().any(|a| a.eq_ignore_ascii_case(addr))
                    });
                }
                HeaderName::ListArchive
                | HeaderName::ListHelp
                | HeaderName::ListId
                | HeaderName::ListOwner
                | HeaderName::ListPost
                | HeaderName::ListSubscribe
                | HeaderName::ListUnsubscribe => {
                    // Do not send vacation responses to lists
                    return TestResult::Bool(false);
                }
                HeaderName::Received => {
                    received_count += 1;
                }
                HeaderName::Other(header_name) => {
                    if header_name.eq_ignore_ascii_case("Auto-Submitted") {
                        if header
                            .value
                            .as_text()
                            .is_none_or(|v| !v.eq_ignore_ascii_case("no"))
                        {
                            return TestResult::Bool(false);
                        }
                    } else if header_name.eq_ignore_ascii_case("X-Auto-Response-Suppress") {
                        if header.value.as_text().is_some_and(|v| {
                            v.to_ascii_lowercase()
                                .split(',')
                                .any(|v| ["all", "oof"].contains(&v.trim()))
                        }) {
                            return TestResult::Bool(false);
                        }
                    } else if header_name.eq_ignore_ascii_case("Precedence")
                        && header
                            .value
                            .as_text()
                            .is_some_and(|v| v.eq_ignore_ascii_case("bulk"))
                    {
                        return TestResult::Bool(false);
                    }
                }
                _ => (),
            }
        }

        // No user address found in header or possible loop
        if found_rcpt && received_count <= ctx.runtime.max_received_headers {
            TestResult::Event {
                event: Event::DuplicateId {
                    id: if let Some(handle) = &self.handle {
                        format!("_v{}{}", from, ctx.eval_value(handle).to_string())
                    } else {
                        format!("_v{}{}", from, ctx.eval_value(&self.reason).to_string())
                    },
                    expiry: match &self.period {
                        Period::Days(days) => days * 86400,
                        Period::Seconds(seconds) => *seconds,
                        Period::Default => ctx.runtime.default_vacation_expiry,
                    },
                    last: false,
                },
                is_not: true,
            }
        } else {
            TestResult::Bool(false)
        }
    }
}

impl Vacation {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut vacation_to = Cow::from("");

        for (name, value) in &ctx.envelope {
            if !value.is_empty() && name == &Envelope::From {
                vacation_to = value.to_string();
                break;
            }
        }

        // Check headers
        let mut vacation_subject = if let Some(subject) = &self.subject {
            ctx.eval_value(subject)
        } else {
            "".into()
        };

        // Check headers
        let mut message_id = None;
        let mut vacation_to_full = None;
        let mut references = None;
        for header in &ctx.message.parts[0].headers {
            match &header.name {
                HeaderName::Subject if vacation_subject.is_empty() => {
                    if let Some(subject) = header.value.as_text() {
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
                HeaderName::MessageId => {
                    message_id = header.value.as_text();
                }
                HeaderName::References => {
                    if header.offset_start > 0 {
                        references = (&ctx.message.raw_message
                            [header.offset_start as usize..header.offset_end as usize])
                            .into();
                    }
                }
                HeaderName::From | HeaderName::Sender => {
                    if matches!(&header.value, HeaderValue::Address(address) if address.contains(vacation_to.as_ref()))
                        && header.offset_start > 0
                    {
                        vacation_to_full = (&ctx.message.raw_message
                            [header.offset_start as usize..header.offset_end as usize])
                            .into();
                    }
                }
                _ => (),
            }
        }

        // Build message
        let vacation_from = if let Some(from) = &self.from {
            ctx.eval_value(from)
        } else if !ctx.user_address.is_empty() {
            ctx.user_from_field().into()
        } else if let Some(addr) =
            ctx.envelope
                .iter()
                .find_map(|(n, v)| if n == &Envelope::To { Some(v) } else { None })
        {
            addr.to_string().into()
        } else {
            "".into()
        };
        if vacation_subject.is_empty() {
            vacation_subject = ctx.runtime.vacation_default_subject.as_ref().into();
        }
        let vacation_body = ctx.eval_value(&self.reason);
        let message_len = vacation_body.len()
            + vacation_from.len()
            + vacation_to_full
                .as_ref()
                .map_or(vacation_to.len(), |t| t.len())
            + vacation_subject.len()
            + message_id.as_ref().map_or(0, |m| m.len() * 2)
            + references.as_ref().map_or(0, |m| m.len())
            + 160;

        let mut message = Vec::with_capacity(message_len);
        write_header(&mut message, "From: ", vacation_from.to_string().as_ref());
        if let Some(vacation_to_full) = vacation_to_full {
            message.extend_from_slice(b"To:");
            message.extend_from_slice(vacation_to_full);
        } else {
            write_header(&mut message, "To: ", vacation_to.to_string().as_ref());
        }
        write_header(
            &mut message,
            "Subject: ",
            vacation_subject.to_string().as_ref(),
        );
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
        generate_message_id_header(&mut message, &ctx.runtime.local_hostname).unwrap();
        message.extend_from_slice(b"\r\n");

        write_header(&mut message, "Auto-Submitted: ", "auto-replied");
        if !self.mime {
            message.extend_from_slice(b"Content-type: text/plain; charset=utf-8\r\n\r\n");
        }
        message.extend_from_slice(vacation_body.to_string().as_bytes());

        // Add action
        let mut events = Vec::with_capacity(3);
        ctx.last_message_id += 1;
        ctx.num_out_messages += 1;
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
                folder: ctx.eval_value(&fcc.mailbox).to_string().into_owned(),
                flags: ctx.get_local_flags(&fcc.flags),
                mailbox_id: fcc
                    .mailbox_id
                    .as_ref()
                    .map(|m| ctx.eval_value(m).to_string().into_owned()),
                special_use: fcc
                    .special_use
                    .as_ref()
                    .map(|s| ctx.eval_value(s).to_string().into_owned()),
                create: fcc.create,
                message_id: ctx.last_message_id,
            });
        }
        ctx.queued_events = events.into_iter();
    }
}

fn write_header(buf: &mut Vec<u8>, name: &str, value: &str) {
    buf.extend_from_slice(name.as_bytes());
    buf.extend_from_slice(value.as_bytes());
    buf.extend_from_slice(b"\r\n");
}
