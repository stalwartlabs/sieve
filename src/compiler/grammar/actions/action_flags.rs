use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use super::action_set::Variable;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EditFlags {
    pub action: Action,
    pub name: Option<Variable>,
    pub flags: Vec<StringItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Action {
    Set,
    Add,
    Remove,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_flag_action(&mut self, word: Word) -> Result<(), CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        let action = match word {
            Word::SetFlag => Action::Set,
            Word::AddFlag => Action::Add,
            Word::RemoveFlag => Action::Remove,
            _ => unreachable!(),
        };

        let instruction = Instruction::EditFlags(
            match (
                &token_info.token,
                self.tokens.peek().map(|r| r.map(|t| &t.token)),
            ) {
                (
                    Token::StringConstant(_),
                    Some(Ok(
                        Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
                    )),
                ) => EditFlags {
                    name: self.parse_variable_name(token_info)?.into(),
                    flags: self.parse_strings()?,
                    action,
                },
                (Token::BracketOpen, _)
                | (
                    Token::StringConstant(_) | Token::StringVariable(_),
                    Some(Ok(Token::Semicolon)),
                ) => EditFlags {
                    name: None,
                    flags: self.parse_strings_token(token_info)?,
                    action,
                },
                _ => {
                    return Err(token_info.invalid("invalid parameters on flag action"));
                }
            },
        );

        self.instructions.push(instruction);

        Ok(())
    }
}
