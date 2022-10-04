use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Keep {
    pub flags: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_keep(&mut self) -> Result<(), CompileError> {
        let cmd = Command::Keep(Keep {
            flags: match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
                Some(Ok(Token::Tag(Word::Flags))) => {
                    self.tokens.next();
                    self.parse_strings(false)?
                }
                _ => Vec::new(),
            },
        });
        self.commands.push(cmd);
        Ok(())
    }
}
