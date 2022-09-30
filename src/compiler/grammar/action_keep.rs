use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Keep {
    pub flags: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_keep(&mut self) -> Result<Keep, CompileError> {
        Ok(Keep {
            flags: match self.peek().map(|r| r.map(|t| &t.token)) {
                Some(Ok(Token::Tag(Word::Flags))) => {
                    self.next();
                    self.parse_strings(false)?
                }
                _ => Vec::new(),
            },
        })
    }
}
