use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FlagAction {
    pub varname: Option<StringItem>,
    pub flags: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_flag_action(&mut self, word: Word) -> Result<(), CompileError> {
        let mut maybe_flags = self.parse_strings()?;

        let action = match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen)) => {
                if maybe_flags.len() == 1 {
                    FlagAction {
                        varname: maybe_flags.pop(),
                        flags: self.parse_strings()?,
                    }
                } else {
                    return Err(self
                        .tokens
                        .unwrap_next()?
                        .invalid("variable name cannot be a list"));
                }
            }
            _ => FlagAction {
                varname: None,
                flags: maybe_flags,
            },
        };

        self.commands.push(match word {
            Word::SetFlag => Command::SetFlag(action),
            Word::AddFlag => Command::AddFlag(action),
            Word::RemoveFlag => Command::RemoveFlag(action),
            _ => unreachable!(),
        });
        Ok(())
    }
}
