/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::super::tests::TestResult;
use crate::{
    Context, Envelope, Sieve,
    bytecode::ops,
    compiler::grammar::{
        AddressPart,
        actions::action_redirect::{ByTime, Notify, Ret},
    },
    runtime::{
        RuntimeError,
        handler::{Action, Handler, MessageSource, Recipient},
    },
};
use mail_builder::headers::{date::Date, message_id::generate_message_id_header};
use mail_parser::{HeaderName, HeaderValue};
use std::borrow::Cow;

pub(crate) const MAX_SUBJECT_LEN: usize = 256;

const PERIOD_DAYS: u8 = 0;
const PERIOD_SECONDS: u8 = 1;

impl<'x> Context<'x> {
    pub(crate) fn test_vacation<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestVacation,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let mut from = String::new();
        let mut user_addresses: Vec<Cow<'_, str>> = Vec::new();

        if self.num_out_messages >= self.runtime.max_out_messages {
            return Ok(TestResult::Bool(false));
        }

        for (name, value) in &self.envelope {
            if !value.is_empty() {
                match name {
                    Envelope::From => {
                        from = value.to_string().to_ascii_lowercase();
                    }
                    Envelope::To if !self.runtime.vacation_use_orig_rcpt => {
                        user_addresses.push(value.to_string());
                    }
                    Envelope::Orcpt if self.runtime.vacation_use_orig_rcpt => {
                        user_addresses.push(value.to_string());
                    }
                    _ => (),
                }
            }
        }

        for address in self.eval_values(script, test.addresses)? {
            let address = address.into_string();
            if !address.is_empty() {
                user_addresses.push(address);
            }
        }
        if !self.user_address.is_empty() {
            user_addresses.push(Cow::Borrowed(self.user_address.as_ref()));
        }

        if from.is_empty()
            || user_addresses.is_empty()
            || from.starts_with("mailer-daemon")
            || from.starts_with("owner-")
            || from.contains("-request@")
            || user_addresses.iter().any(|a| a.eq_ignore_ascii_case(&from))
        {
            return Ok(TestResult::Bool(false));
        }

        let mut found_rcpt = false;
        let mut received_count = 0;
        for header in &self.message.root_part().headers {
            match &header.name {
                HeaderName::To
                | HeaderName::Cc
                | HeaderName::Bcc
                | HeaderName::ResentTo
                | HeaderName::ResentBcc
                | HeaderName::ResentCc
                    if !found_rcpt =>
                {
                    found_rcpt = self.find_addresses(header, &AddressPart::All, |addr| {
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
                    return Ok(TestResult::Bool(false));
                }
                HeaderName::Received => {
                    received_count += 1;
                }
                HeaderName::AutoSubmitted => {
                    if header
                        .value
                        .as_text()
                        .is_none_or(|v| !v.eq_ignore_ascii_case("no"))
                    {
                        return Ok(TestResult::Bool(false));
                    }
                }
                HeaderName::Other(header_name) => {
                    if header_name.eq_ignore_ascii_case("X-Auto-Response-Suppress") {
                        if header.value.as_text().is_some_and(|v| {
                            v.to_ascii_lowercase()
                                .split(',')
                                .any(|v| ["all", "oof"].contains(&v.trim()))
                        }) {
                            return Ok(TestResult::Bool(false));
                        }
                    } else if header_name.eq_ignore_ascii_case("Precedence")
                        && header
                            .value
                            .as_text()
                            .is_some_and(|v| v.eq_ignore_ascii_case("bulk"))
                    {
                        return Ok(TestResult::Bool(false));
                    }
                }
                _ => (),
            }
        }

        if found_rcpt && received_count <= self.runtime.max_received_headers {
            let suffix = match self.eval_opt(script, test.handle)? {
                Some(handle) => handle,
                None => self.eval_value(script, test.reason)?,
            };
            let id = format!("_v{}{}", from, suffix.to_string());
            let expiry = match test.period.kind {
                PERIOD_DAYS => test.period.value.saturating_mul(86400),
                PERIOD_SECONDS => test.period.value,
                _ => self.runtime.default_vacation_expiry,
            };
            TestResult::from_reply(handler.duplicate_id(self, &id, expiry, false), true)
        } else {
            Ok(TestResult::Bool(false))
        }
    }

    pub(crate) fn exec_vacation(
        &mut self,
        script: &'x Sieve<'x>,
        vacation: &ops::Vacation,
    ) -> Result<(), RuntimeError> {
        let vacation_to = self
            .envelope
            .iter()
            .find(|(name, value)| !value.is_empty() && name == &Envelope::From)
            .map_or(Cow::Borrowed(""), |(_, value)| value.clone().into_string());

        let mut vacation_subject = self
            .eval_opt(script, vacation.subject)?
            .map_or(Cow::Borrowed(""), |subject| subject.into_string());

        let mut message_id = None;
        let mut vacation_to_full = None;
        let mut references = None;
        for header in &self.message.root_part().headers {
            match &header.name {
                HeaderName::Subject if vacation_subject.is_empty() => {
                    if let Some(subject) = header.value.as_text() {
                        let mut vacation_subject_ = String::with_capacity(MAX_SUBJECT_LEN);
                        let mut iter = self
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
                HeaderName::References if header.offset_start > 0 => {
                    references = self
                        .message
                        .raw_message
                        .get(header.offset_start as usize..header.offset_end as usize);
                }
                HeaderName::From | HeaderName::Sender
                    if matches!(&header.value, HeaderValue::Address(address) if address.contains(vacation_to.as_ref()))
                        && header.offset_start > 0 =>
                {
                    vacation_to_full = self
                        .message
                        .raw_message
                        .get(header.offset_start as usize..header.offset_end as usize);
                }
                _ => (),
            }
        }

        let vacation_from = if let Some(from) = self.eval_opt(script, vacation.from)? {
            from.into_string()
        } else if !self.user_address.is_empty() {
            self.user_from_field().into()
        } else if let Some(addr) = self
            .envelope
            .iter()
            .find_map(|(n, v)| if n == &Envelope::To { Some(v) } else { None })
        {
            addr.to_string()
        } else {
            "".into()
        };
        if vacation_subject.is_empty() {
            vacation_subject = self.runtime.vacation_default_subject.as_ref().into();
        }
        let vacation_body = self.eval_value(script, vacation.reason)?.into_string();
        let message_len = vacation_body.len()
            + vacation_from.len()
            + vacation_to_full.map_or(vacation_to.len(), |t| t.len())
            + vacation_subject.len()
            + message_id.map_or(0, |m| m.len() * 2)
            + references.map_or(0, |m| m.len())
            + 160;

        let mut message = Vec::with_capacity(message_len);
        write_header(&mut message, "From: ", vacation_from.as_ref());
        if let Some(vacation_to_full) = vacation_to_full {
            message.extend_from_slice(b"To:");
            message.extend_from_slice(vacation_to_full);
        } else {
            write_header(&mut message, "To: ", vacation_to.as_ref());
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
        generate_message_id_header(&mut message, &self.runtime.local_hostname)
            .expect("writing to a Vec cannot fail");
        message.extend_from_slice(b"\r\n");

        write_header(&mut message, "Auto-Submitted: ", "auto-replied");
        if !vacation.mime {
            message.extend_from_slice(b"Content-type: text/plain; charset=utf-8\r\n\r\n");
        }
        message.extend_from_slice(vacation_body.as_bytes());

        let recipient = self.intern_cow(vacation_to);
        self.last_message_id += 1;
        self.num_out_messages += 1;
        self.actions.push(Action::CreatedMessage {
            message_id: self.last_message_id,
            message,
        });
        self.actions.push(Action::SendMessage {
            source: MessageSource::Vacation,
            recipient: Recipient::Address(recipient),
            notify: Notify::Never,
            return_of_content: Ret::Default,
            by_time: ByTime::None,
            message_id: self.last_message_id,
        });

        if vacation.fcc.present {
            let fcc = &vacation.fcc;
            let action = Action::FileInto {
                folder: self.eval_str(script, fcc.mailbox)?,
                flags: self.get_local_flags(script, fcc.flags)?,
                mailbox_id: self.eval_opt_str(script, fcc.mailbox_id)?,
                special_use: self.eval_opt_str(script, fcc.special_use)?,
                create: fcc.create,
                message_id: self.last_message_id,
            };
            self.actions.push(action);
        }
        Ok(())
    }
}

fn write_header(buf: &mut Vec<u8>, name: &str, value: &str) {
    buf.extend_from_slice(name.as_bytes());
    buf.extend_from_slice(value.as_bytes());
    buf.extend_from_slice(b"\r\n");
}
