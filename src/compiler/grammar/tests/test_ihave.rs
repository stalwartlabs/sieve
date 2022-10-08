use serde::{Deserialize, Serialize};

use crate::compiler::grammar::instruction::{CompilerState, Instruction};
use crate::compiler::grammar::Capability;
use crate::compiler::lexer::string::StringItem;
use crate::compiler::CompileError;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestIhave {
    pub capabilities: Vec<Capability>,
    pub is_not: bool,
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
            is_not: false,
        }))
    }

    pub(crate) fn parse_error(&mut self) -> Result<(), CompileError> {
        let cmd = Instruction::Error(Error {
            message: self.parse_string()?,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
