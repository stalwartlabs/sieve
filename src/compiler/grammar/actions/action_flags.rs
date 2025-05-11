/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{word::Word, Token},
    CompileError, ErrorType, Value, VariableType,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct EditFlags {
    pub action: Action,
    pub name: Option<VariableType>,
    pub flags: Vec<Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum Action {
    Set,
    Add,
    Remove,
}

impl CompilerState<'_> {
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
                    name: self.parse_variable_name(token_info, false)?.into(),
                    flags: self.parse_strings(false)?,
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
                    return Err(token_info.custom(ErrorType::InvalidArguments));
                }
            },
        );

        self.instructions.push(instruction);

        Ok(())
    }
}
