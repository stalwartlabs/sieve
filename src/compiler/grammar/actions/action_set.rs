/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    Envelope,
    compiler::{
        CompileError, ErrorType, Value, VariableType,
        grammar::{
            expr::Expression,
            instruction::{CompilerState, Instruction},
        },
        lexer::{Token, tokenizer::TokenInfo, word::Word},
    },
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
#[repr(u8)]
pub(crate) enum Modifier {
    Lower = 0,
    Upper = 1,
    LowerFirst = 2,
    UpperFirst = 3,
    QuoteWildcard = 4,
    QuoteRegex = 5,
    EncodeUrl = 6,
    Length = 7,
    Replace(Box<Replacement>) = 8,
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
pub(crate) struct Replacement {
    pub find: Value,
    pub replace: Value,
}

impl Modifier {
    pub fn order(&self) -> usize {
        match self {
            Modifier::Lower => 41,
            Modifier::Upper => 40,
            Modifier::LowerFirst => 31,
            Modifier::UpperFirst => 30,
            Modifier::QuoteWildcard => 20,
            Modifier::QuoteRegex => 21,
            Modifier::EncodeUrl => 15,
            Modifier::Length => 10,
            Modifier::Replace(_) => 40,
        }
    }
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
pub(crate) struct Set {
    pub modifiers: Box<[Modifier]>,
    pub name: VariableType,
    pub value: Value,
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
pub(crate) struct Let {
    pub name: VariableType,
    pub expr: Box<[Expression]>,
}

impl CompilerState<'_> {
    pub(crate) fn parse_set(&mut self) -> Result<(), CompileError> {
        let mut modifiers = Vec::new();
        let mut name = None;
        let mut is_local = false;
        let value;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Lower
                    | Word::Upper
                    | Word::LowerFirst
                    | Word::UpperFirst
                    | Word::QuoteWildcard
                    | Word::QuoteRegex
                    | Word::Length
                    | Word::EncodeUrl),
                ) => {
                    let modifier = word.into();
                    if !modifiers.contains(&modifier) {
                        modifiers.push(modifier);
                    }
                }
                Token::Tag(Word::Replace) => {
                    let find = self.tokens.unwrap_next()?;
                    let replace = self.tokens.unwrap_next()?;
                    modifiers.push(Modifier::Replace(Box::new(Replacement {
                        find: self.parse_string_token(find)?,
                        replace: self.parse_string_token(replace)?,
                    })));
                }
                Token::Tag(Word::Local) => {
                    is_local = true;
                }
                _ => {
                    if name.is_none() {
                        name = self.parse_variable_name(token_info, is_local)?.into();
                    } else {
                        value = self.parse_string_token(token_info)?;
                        break;
                    }
                }
            }
        }

        modifiers.sort_unstable_by_key(|m| std::cmp::Reverse(m.order()));

        self.instructions.push(Instruction::Set(Box::new(Set {
            modifiers: modifiers.into(),
            name: name.unwrap(),
            value,
        })));
        Ok(())
    }

    pub(crate) fn parse_let(&mut self) -> Result<(), CompileError> {
        let name = self.tokens.unwrap_next()?;
        let name = self.parse_variable_name(name, false)?;
        let expr = self.parse_expr()?;

        self.instructions.push(Instruction::Let(Box::new(Let {
            name,
            expr: expr.into(),
        })));
        Ok(())
    }

    pub(crate) fn parse_variable_name(
        &mut self,
        token_info: TokenInfo,
        register_as_local: bool,
    ) -> Result<VariableType, CompileError> {
        match token_info.token {
            Token::StringConstant(value) => self
                .register_variable(value.into_string(), register_as_local)
                .map_err(|error_type| CompileError {
                    line_num: token_info.line_num,
                    line_pos: token_info.line_pos,
                    error_type,
                }),
            _ => Err(token_info.custom(ErrorType::ExpectedConstantString)),
        }
    }

    pub(crate) fn register_variable(
        &mut self,
        name: String,
        register_as_local: bool,
    ) -> Result<VariableType, ErrorType> {
        let name = name.to_lowercase();
        if let Some((namespace, part)) = name.split_once('.') {
            let mut variable = None;
            hashify::fnc_map!(namespace.as_bytes(),
                "global" => {
                    variable = VariableType::Global(part.to_string()).into();
                },
                "t" => {
                    variable = VariableType::Global(part.to_string()).into();
                },
                "envelope" => {
                    variable = Envelope::try_from(part).ok().map(VariableType::Envelope);
                },
                _ => {}
            );

            variable.ok_or_else(|| ErrorType::InvalidNamespace(namespace.to_string()))
        } else {
            Ok(if !self.is_var_global(&name) {
                VariableType::Local(self.register_local_var(name, register_as_local))
            } else {
                VariableType::Global(name)
            })
        }
    }
}

impl From<Word> for Modifier {
    fn from(word: Word) -> Self {
        match word {
            Word::Lower => Modifier::Lower,
            Word::Upper => Modifier::Upper,
            Word::LowerFirst => Modifier::LowerFirst,
            Word::UpperFirst => Modifier::UpperFirst,
            Word::QuoteWildcard => Modifier::QuoteWildcard,
            Word::QuoteRegex => Modifier::QuoteRegex,
            Word::Length => Modifier::Length,
            Word::EncodeUrl => Modifier::EncodeUrl,
            _ => unreachable!(),
        }
    }
}
