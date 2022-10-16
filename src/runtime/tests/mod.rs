use crate::{
    compiler::grammar::{test::Test, Capability},
    Context, Event,
};

use super::RuntimeError;

pub mod comparator;
pub mod glob;
pub mod mime;
pub mod test_address;
pub mod test_body;
pub mod test_date;
pub mod test_envelope;
pub mod test_exists;
pub mod test_hasflag;
pub mod test_header;
pub mod test_size;
pub mod test_string;

pub(crate) enum TestResult {
    Bool(bool),
    Event { event: Event, is_not: bool },
    Error(RuntimeError),
}

impl Test {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        TestResult::Bool(match &self {
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
            Test::Duplicate(_) => todo!(),
            Test::NotifyMethodCapability(_) => todo!(),
            Test::ValidNotifyMethod(_) => todo!(),
            Test::Environment(test) => test.exec(ctx, true),
            Test::ValidExtList(_) => todo!(),
            Test::Ihave(test) => {
                test.capabilities.iter().all(|c| {
                    ![Capability::Variables, Capability::EncodedCharacter].contains(c)
                        && ctx.runtime.allowed_capabilities.contains(c)
                }) ^ test.is_not
            }
            Test::MailboxExists(test) => {
                return TestResult::Event {
                    event: Event::MailboxExists {
                        names: ctx.eval_strings_owned(&test.mailbox_names),
                    },
                    is_not: test.is_not,
                };
            }
            Test::Metadata(_) => todo!(),
            Test::MetadataExists(_) => todo!(),
            Test::ServerMetadata(_) => todo!(),
            Test::ServerMetadataExists(_) => todo!(),
            Test::MailboxIdExists(_) => todo!(),
            Test::SpamTest(_) => todo!(),
            Test::VirusTest(_) => todo!(),
            Test::SpecialUseExists(test) => {
                return TestResult::Event {
                    event: Event::SpecialUseExists {
                        mailbox: test
                            .mailbox
                            .as_ref()
                            .map(|m| ctx.eval_string(m).into_owned()),
                        attributes: ctx.eval_strings_owned(&test.attributes),
                    },
                    is_not: test.is_not,
                };
            }
            Test::Convert(_) => todo!(),
            Test::True => true,
            Test::False => false,
            Test::Invalid(invalid) => {
                return TestResult::Error(RuntimeError::InvalidInstruction(invalid.clone()))
            }
            #[cfg(test)]
            Test::External((command, params, is_not)) => {
                return TestResult::Event {
                    event: Event::TestCommand {
                        command: command.clone(),
                        params: params
                            .iter()
                            .map(|p| ctx.eval_string(p).into_owned())
                            .collect(),
                    },
                    is_not: *is_not,
                };
            }
        })
    }
}
