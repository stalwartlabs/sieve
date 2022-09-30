use serde::{Deserialize, Serialize};

use crate::{
    compiler::{lexer::tokenizer::Tokenizer, CompileError},
    runtime::StringItem,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMailboxIdExists {
    pub mailbox_ids: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_mailboxidexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MailboxIdExists(TestMailboxIdExists {
            mailbox_ids: self.parse_strings(false)?,
        }))
    }
}
