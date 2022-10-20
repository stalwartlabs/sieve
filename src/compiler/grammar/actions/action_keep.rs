use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
        Capability,
    },
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Keep {
    pub flags: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_keep(&mut self) -> Result<(), CompileError> {
        let cmd = Instruction::Keep(Keep {
            flags: match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
                Some(Ok(Token::Tag(Word::Flags))) => {
                    let token_info = self.tokens.next().unwrap().unwrap();
                    self.validate_argument(
                        0,
                        Capability::Imap4Flags.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    self.parse_strings()?
                }
                _ => Vec::new(),
            },
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
