/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{word::Word, Token},
    CompileError, Value, VariableType,
};

use super::action_set::Modifier;

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct ForEveryPart {
    pub jz_pos: usize,
}

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct Replace {
    pub subject: Option<Value>,
    pub from: Option<Value>,
    pub replacement: Value,
    pub mime: bool,
}

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct Enclose {
    pub subject: Option<Value>,
    pub headers: Vec<Value>,
    pub value: Value,
}

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct ExtractText {
    pub modifiers: Vec<Modifier>,
    pub first: Option<usize>,
    pub name: VariableType,
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
pub(crate) enum MimeOpts<T> {
    Type,
    Subtype,
    ContentType,
    Param(Vec<T>),
    None,
}

impl CompilerState<'_> {
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
                    headers = self.parse_strings(false)?;
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
        let mut is_local = false;

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
                Token::Tag(Word::Local) => {
                    is_local = true;
                }
                _ => {
                    name = self.parse_variable_name(token_info, is_local)?;
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
            Word::Param => MimeOpts::Param(self.parse_strings(false)?),
            _ => MimeOpts::None,
        })
    }
}
