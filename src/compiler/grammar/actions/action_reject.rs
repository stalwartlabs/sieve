use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::string::StringItem,
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Reject {
    pub ereject: bool,
    pub reason: StringItem,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_reject(&mut self, ereject: bool) -> Result<(), CompileError> {
        let cmd = Instruction::Reject(Reject {
            ereject,
            reason: self.parse_string()?,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
