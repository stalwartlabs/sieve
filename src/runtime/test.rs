use crate::{
    compiler::grammar::test::{BoolOp, Test},
    Context, Event,
};

use super::RuntimeError;

pub(crate) enum TestResult {
    Bool(bool),
    Event(Event),
    Error(RuntimeError),
}

impl<'x, 'y> Context<'x, 'y> {
    pub(crate) fn eval_test(&self, op: &BoolOp) -> TestResult {
        TestResult::Bool(
            match &op.test {
                Test::True => true,
                Test::False => false,
                Test::Address(_) => todo!(),
                Test::Envelope(_) => todo!(),
                Test::Exists(_) => todo!(),
                Test::Header(_) => todo!(),
                Test::Size(_) => todo!(),
                Test::Body(_) => todo!(),
                Test::Convert(_) => todo!(),
                Test::Date(_) => todo!(),
                Test::CurrentDate(_) => todo!(),
                Test::Duplicate(_) => todo!(),
                Test::String(_) => todo!(),
                Test::NotifyMethodCapability(_) => todo!(),
                Test::ValidNotifyMethod(_) => todo!(),
                Test::Environment(_) => todo!(),
                Test::ValidExtList(_) => todo!(),
                Test::Ihave(ihave) => ihave
                    .capabilities
                    .iter()
                    .all(|c| self.runtime.allowed_capabilities.contains(c)),
                Test::HasFlag(_) => todo!(),
                Test::MailboxExists(me) => {
                    return TestResult::Event(Event::MailboxExists {
                        names: self.eval_strings_owned(&me.mailbox_names),
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
                Test::Invalid(invalid) => {
                    return TestResult::Error(RuntimeError::InvalidInstruction(invalid.clone()))
                }
            } ^ op.is_not,
        )
    }
}
