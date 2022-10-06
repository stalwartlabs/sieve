use crate::{
    compiler::grammar::test::{BoolOp, Test},
    Context, Event,
};

use super::RuntimeError;

pub mod glob;
pub mod match_type;
pub mod test_string;

pub(crate) enum TestResult {
    Bool(bool),
    Event(Event),
    Error(RuntimeError),
}

impl BoolOp {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        TestResult::Bool(match &self.test {
            Test::Address(_) => todo!(),
            Test::Envelope(_) => todo!(),
            Test::Exists(_) => todo!(),
            Test::Header(_) => todo!(),
            Test::Size(_) => todo!(),
            Test::Body(_) => todo!(),
            Test::String(string) => string.exec(ctx, self.is_not),
            Test::Convert(_) => todo!(),
            Test::Date(_) => todo!(),
            Test::CurrentDate(_) => todo!(),
            Test::Duplicate(_) => todo!(),
            Test::NotifyMethodCapability(_) => todo!(),
            Test::ValidNotifyMethod(_) => todo!(),
            Test::Environment(_) => todo!(),
            Test::ValidExtList(_) => todo!(),
            Test::Ihave(ihave) => {
                ihave
                    .capabilities
                    .iter()
                    .all(|c| ctx.runtime.allowed_capabilities.contains(c))
                    ^ self.is_not
            }
            Test::HasFlag(_) => todo!(),
            Test::MailboxExists(me) => {
                return TestResult::Event(Event::MailboxExists {
                    names: ctx.eval_strings_owned(&me.mailbox_names),
                });
            }
            Test::Metadata(_) => todo!(),
            Test::MetadataExists(_) => todo!(),
            Test::ServerMetadata(_) => todo!(),
            Test::ServerMetadataExists(_) => todo!(),
            Test::MailboxIdExists(_) => todo!(),
            Test::SpamTest(_) => todo!(),
            Test::VirusTest(_) => todo!(),
            Test::SpecialUseExists(_) => todo!(),
            Test::True => true ^ self.is_not,
            Test::False => false ^ self.is_not,
            Test::Invalid(invalid) => {
                return TestResult::Error(RuntimeError::InvalidInstruction(invalid.clone()))
            }
        })
    }
}
