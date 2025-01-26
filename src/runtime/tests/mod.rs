/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    compiler::grammar::{test::Test, Capability},
    Context, Event, Mailbox,
};

use super::RuntimeError;

pub mod comparator;
pub mod glob;
pub mod mime;
pub mod test_address;
pub mod test_body;
pub mod test_date;
pub mod test_duplicate;
pub mod test_envelope;
pub mod test_exists;
pub mod test_extlists;
pub mod test_hasflag;
pub mod test_header;
pub mod test_metadata;
pub mod test_notify;
pub mod test_size;
pub mod test_spamtest;
pub mod test_string;

pub(crate) enum TestResult {
    Bool(bool),
    Event { event: Event, is_not: bool },
    Error(RuntimeError),
}

impl Test {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        match &self {
            Test::Header(test) => test.exec(ctx),
            Test::Address(test) => test.exec(ctx),
            Test::Envelope(test) => test.exec(ctx),
            Test::Exists(test) => test.exec(ctx),
            Test::Size(test) => test.exec(ctx),
            Test::Body(test) => test.exec(ctx),
            Test::String(test) => test.exec(ctx, false),
            Test::HasFlag(test) => test.exec(ctx),
            Test::Date(test) => test.exec(ctx),
            Test::CurrentDate(test) => test.exec(ctx),
            Test::Duplicate(test) => test.exec(ctx),
            Test::NotifyMethodCapability(test) => test.exec(ctx),
            Test::ValidNotifyMethod(test) => test.exec(ctx),
            Test::Environment(test) => test.exec(ctx, true),
            Test::ValidExtList(test) => test.exec(ctx),
            Test::Ihave(test) => TestResult::Bool(
                test.capabilities.iter().all(|c| {
                    ![Capability::Variables, Capability::EncodedCharacter].contains(c)
                        && ctx.runtime.allowed_capabilities.contains(c)
                }) ^ test.is_not,
            ),
            Test::MailboxExists(test) => TestResult::Event {
                event: Event::MailboxExists {
                    mailboxes: test
                        .mailbox_names
                        .iter()
                        .map(|m| Mailbox::Name(ctx.eval_value(m).to_string().into_owned()))
                        .collect(),
                    special_use: Vec::new(),
                },
                is_not: test.is_not,
            },
            Test::Vacation(test) => test.exec(ctx),
            Test::Metadata(test) => test.exec(ctx),
            Test::MetadataExists(test) => test.exec(ctx),
            Test::MailboxIdExists(test) => TestResult::Event {
                event: Event::MailboxExists {
                    mailboxes: test
                        .mailbox_ids
                        .iter()
                        .map(|m| Mailbox::Id(ctx.eval_value(m).to_string().into_owned()))
                        .collect(),
                    special_use: Vec::new(),
                },
                is_not: test.is_not,
            },
            Test::SpamTest(test) => test.exec(ctx),
            Test::VirusTest(test) => test.exec(ctx),
            Test::SpecialUseExists(test) => TestResult::Event {
                event: Event::MailboxExists {
                    mailboxes: if let Some(mailbox) = &test.mailbox {
                        vec![Mailbox::Name(
                            ctx.eval_value(mailbox).to_string().into_owned(),
                        )]
                    } else {
                        Vec::new()
                    },
                    special_use: ctx.eval_values_owned(&test.attributes),
                },
                is_not: test.is_not,
            },
            Test::Convert(test) => test.exec(ctx),
            Test::True => TestResult::Bool(true),
            Test::False => TestResult::Bool(false),
            Test::Invalid(invalid) => {
                TestResult::Error(RuntimeError::InvalidInstruction(invalid.clone()))
            }
            #[cfg(test)]
            Test::TestCmd { arguments, is_not } => TestResult::Event {
                event: Event::Function {
                    id: u32::MAX,
                    arguments: arguments
                        .iter()
                        .map(|s| ctx.eval_value(s).to_owned())
                        .collect(),
                },
                is_not: *is_not,
            },
        }
    }
}
