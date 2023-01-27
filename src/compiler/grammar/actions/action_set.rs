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
        lexer::{string::StringItem, tokenizer::TokenInfo, word::Word, Token},
        CompileError, ErrorType,
    },
    runtime::string::IntoString,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Modifier {
    Lower = 41,
    Upper = 40,
    LowerFirst = 31,
    UpperFirst = 30,
    QuoteWildcard = 20,
    QuoteRegex = 21,
    EncodeUrl = 15,
    Length = 10,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Set {
    pub modifiers: Vec<Modifier>,
    pub name: Variable,
    pub value: StringItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Variable {
    Local(usize),
    Global(String),
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_set(&mut self) -> Result<(), CompileError> {
        let mut modifiers = Vec::new();
        let mut name = None;
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
                _ => {
                    if name.is_none() {
                        name = self.parse_variable_name(token_info)?.into();
                    } else {
                        value = self.parse_string_token(token_info)?;
                        break;
                    }
                }
            }
        }

        modifiers.sort_unstable_by(|a: &Modifier, b: &Modifier| b.cmp(a));

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
    ) -> Result<Variable, CompileError> {
        match token_info.token {
            Token::StringConstant(value) => {
                self.register_variable(value.into_string())
                    .map_err(|error_type| CompileError {
                        line_num: token_info.line_num,
                        line_pos: token_info.line_pos,
                        error_type,
                    })
            }
            _ => Err(token_info.custom(ErrorType::ExpectedConstantString)),
        }
    }

    pub(crate) fn register_variable(&mut self, name: String) -> Result<Variable, ErrorType> {
        let name = name.to_lowercase();
        if let Some((namespace, name)) = name.split_once('.') {
            if namespace == "global" {
                Ok(Variable::Global(name.to_string()))
            } else {
                Err(ErrorType::InvalidNamespace(namespace.to_string()))
            }
        } else {
            Ok(if !self.is_var_global(&name) {
                Variable::Local(self.register_local_var(name))
            } else {
                Variable::Global(name)
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
