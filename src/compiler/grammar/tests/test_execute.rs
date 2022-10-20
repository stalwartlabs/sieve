use serde::{Deserialize, Serialize};

use crate::compiler::grammar::instruction::{CompilerState, Instruction};
use crate::compiler::lexer::string::StringItem;
use crate::compiler::CompileError;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Execute {
    pub command: StringItem,
    pub arguments: Vec<StringItem>,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Error {
    pub message: StringItem,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_execute(&mut self) -> Result<(), CompileError> {
        let command = Execute {
            command: self.parse_string()?,
            arguments: self.parse_strings()?,
            is_not: false,
        };
        self.instructions.push(Instruction::Execute(command));

        Ok(())
    }

    pub(crate) fn parse_test_execute(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Execute(Execute {
            command: self.parse_string()?,
            arguments: self.parse_strings()?,
            is_not: false,
        }))
    }
}
