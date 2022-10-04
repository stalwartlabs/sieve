use serde::{Deserialize, Serialize};

use crate::compiler::grammar::command::{Command, CompilerState};
use crate::compiler::grammar::Capability;
use crate::compiler::lexer::string::StringItem;
use crate::compiler::CompileError;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestIhave {
    pub capabilities: Vec<Capability>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Error {
    pub message: StringItem,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_ihave(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Ihave(TestIhave {
            capabilities: self
                .parse_static_strings()?
                .into_iter()
                .map(|n| n.into())
                .collect(),
        }))
    }

    pub(crate) fn parse_error(&mut self) -> Result<(), CompileError> {
        let cmd = Command::Error(Error {
            message: self.parse_string()?,
        });
        self.commands.push(cmd);
        Ok(())
    }
}
