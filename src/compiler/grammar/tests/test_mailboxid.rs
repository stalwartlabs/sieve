use serde::{Deserialize, Serialize};

use crate::compiler::grammar::instruction::CompilerState;
use crate::compiler::lexer::string::StringItem;
use crate::compiler::CompileError;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMailboxIdExists {
    pub mailbox_ids: Vec<StringItem>,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_mailboxidexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MailboxIdExists(TestMailboxIdExists {
            mailbox_ids: self.parse_strings()?,
            is_not: false,
        }))
    }
}
