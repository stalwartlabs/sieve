/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        grammar::instruction::{CompilerState, Instruction},
        lexer::{tokenizer::TokenInfo, word::Word, Token},
        CompileError, ErrorType, Value, VariableType,
    },
    Envelope,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Modifier {
    Lower,
    Upper,
    LowerFirst,
    UpperFirst,
    QuoteWildcard,
    QuoteRegex,
    EncodeUrl,
    Length,
    Replace { find: Value, replace: Value },
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
            Modifier::Replace { .. } => 40,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Set {
    pub modifiers: Vec<Modifier>,
    pub name: VariableType,
    pub value: Value,
}

impl<'x> CompilerState<'x> {
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
                    modifiers.push(Modifier::Replace {
                        find: self.parse_string_token(find)?,
                        replace: self.parse_string_token(replace)?,
                    });
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

        self.instructions.push(Instruction::Set(Set {
            modifiers,
            name: name.unwrap(),
            value,
        }));
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
            match namespace {
                "global" | "t" => Ok(VariableType::Global(part.to_string())),
                "envelope" => Envelope::try_from(part)
                    .map(VariableType::Envelope)
                    .map_err(|_| ErrorType::InvalidNamespace(namespace.to_string())),
                _ => Err(ErrorType::InvalidNamespace(namespace.to_string())),
            }
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
