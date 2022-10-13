use crate::{compiler::grammar::test::Test, Context, Event};

use super::RuntimeError;

pub mod comparator;
pub mod glob;
pub mod mime;
pub mod test_address;
pub mod test_body;
pub mod test_envelope;
pub mod test_exists;
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
            Test::String(test) => test.exec(ctx),
            Test::Date(_) => todo!(),
            Test::CurrentDate(_) => todo!(),
            Test::Duplicate(_) => todo!(),
            Test::NotifyMethodCapability(_) => todo!(),
            Test::ValidNotifyMethod(_) => todo!(),
            Test::Environment(_) => todo!(),
            Test::ValidExtList(_) => todo!(),
            Test::Ihave(test) => {
                test.capabilities
                    .iter()
                    .all(|c| ctx.runtime.allowed_capabilities.contains(c))
                    ^ test.is_not
            }
            Test::HasFlag(_) => todo!(),
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
            Test::SpecialUseExists(_) => todo!(),
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
