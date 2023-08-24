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
                        .map(|m| Mailbox::Name(ctx.eval_value(m).into_string()))
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
                        .map(|m| Mailbox::Id(ctx.eval_value(m).into_string()))
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
                        vec![Mailbox::Name(ctx.eval_value(mailbox).into_string())]
                    } else {
                        Vec::new()
                    },
                    special_use: ctx.eval_values_owned(&test.attributes),
                },
                is_not: test.is_not,
            },
            Test::Convert(test) => test.exec(ctx),
            Test::Plugin(test) => TestResult::Event {
                event: ctx.eval_plugin_arguments(test),
                is_not: test.is_not,
            },
            Test::True => TestResult::Bool(true),
            Test::False => TestResult::Bool(false),
            Test::Invalid(invalid) => {
                TestResult::Error(RuntimeError::InvalidInstruction(invalid.clone()))
            }
        }
    }
}
