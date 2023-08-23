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

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{word::Word, Token},
    CompileError, Value, VariableType,
};

use super::action_set::Modifier;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ForEveryPart {
    pub jz_pos: usize,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct Replace {
    pub subject: Option<Value>,
    pub from: Option<Value>,
    pub replacement: Value,
    pub mime: bool,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct Enclose {
    pub subject: Option<Value>,
    pub headers: Vec<Value>,
    pub value: Value,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ExtractText {
    pub modifiers: Vec<Modifier>,
    pub first: Option<usize>,
    pub name: VariableType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MimeOpts<T> {
    Type,
    Subtype,
    ContentType,
    Param(Vec<T>),
    None,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_replace(&mut self) -> Result<(), CompileError> {
        let mut subject = None;
        let mut from = None;
        let replacement;
        let mut mime = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    mime = true;
                }
                Token::Tag(Word::Subject) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    subject = self.parse_string()?.into();
                }
                Token::Tag(Word::From) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    from = self.parse_string()?.into();
                }
                _ => {
                    replacement = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.instructions.push(Instruction::Replace(Replace {
            subject,
            from,
            replacement,
            mime,
        }));
        Ok(())
    }

    pub(crate) fn parse_enclose(&mut self) -> Result<(), CompileError> {
        let mut subject = None;
        let mut headers = Vec::new();
        let value;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Subject) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    subject = self.parse_string()?.into();
                }
                Token::Tag(Word::Headers) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    headers = self.parse_strings()?;
                }
                _ => {
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.instructions.push(Instruction::Enclose(Enclose {
            subject,
            headers,
            value,
        }));
        Ok(())
    }

    pub(crate) fn parse_extracttext(&mut self) -> Result<(), CompileError> {
        let mut modifiers = Vec::new();
        let mut first = None;
        let name;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::First) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    first = self.tokens.expect_number(usize::MAX)?.into();
                }
                Token::Tag(
                    word @ (Word::Lower
                    | Word::Upper
                    | Word::LowerFirst
                    | Word::UpperFirst
                    | Word::QuoteWildcard
                    | Word::QuoteRegex
                    | Word::Length),
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
                _ => {
                    name = self.parse_variable_name(token_info)?;
                    break;
                }
            }
        }

        modifiers.sort_unstable_by_key(|m| std::cmp::Reverse(m.order()));

        self.instructions
            .push(Instruction::ExtractText(ExtractText {
                modifiers,
                first,
                name,
            }));
        Ok(())
    }

    pub(crate) fn parse_mimeopts(&mut self, opts: Word) -> Result<MimeOpts<Value>, CompileError> {
        Ok(match opts {
            Word::Type => MimeOpts::Type,
            Word::Subtype => MimeOpts::Subtype,
            Word::ContentType => MimeOpts::ContentType,
            Word::Param => MimeOpts::Param(self.parse_strings()?),
            _ => MimeOpts::None,
        })
    }
}
