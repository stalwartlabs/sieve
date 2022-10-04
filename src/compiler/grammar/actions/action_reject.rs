use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
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
        let cmd = Command::Reject(Reject {
            ereject,
            reason: self.parse_string()?,
        });
        self.commands.push(cmd);
        Ok(())
    }
}
