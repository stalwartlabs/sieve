use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, Token},
        CompileError,
    },
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FlagAction {
    pub varname: Option<StringItem>,
    pub flags: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_flag_action(&mut self) -> Result<FlagAction, CompileError> {
        let mut maybe_flags = self.parse_strings(false)?;

        match self.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::String(_) | Token::BracketOpen)) => {
                if maybe_flags.len() == 1 {
                    Ok(FlagAction {
                        varname: maybe_flags.pop(),
                        flags: self.parse_strings(false)?,
                    })
                } else {
                    Err(self
                        .unwrap_next()?
                        .invalid("variable name cannot be a list"))
                }
            }
            _ => Ok(FlagAction {
                varname: None,
                flags: maybe_flags,
            }),
        }
    }
}
