/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::HeaderName;


use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
        Capability, Comparator,
    },
    lexer::{word::Word, Token},
    CompileError, ErrorType, Value,
};

use crate::compiler::grammar::MatchType;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct AddHeader {
    pub last: bool,
    pub field_name: Value,
    pub value: Value,
}

/*
      Usage: "deleteheader" [":index" <fieldno: number> [":last"]]
                   [COMPARATOR] [MATCH-TYPE]
                   <field-name: string>
                   [<value-patterns: string-list>]

*/
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct DeleteHeader {
    pub index: Option<i32>,
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub field_name: Value,
    pub value_patterns: Vec<Value>,
    pub mime_anychild: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_addheader(&mut self) -> Result<(), CompileError> {
        let mut field_name = None;
        let value;
        let mut last = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Last) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    last = true;
                }
                _ => {
                    let string = self.parse_string_token(token_info)?;
                    if field_name.is_none() {
                        if let Value::Text(header_name) = &string {
                            if HeaderName::parse(header_name.as_ref()).is_none() {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .custom(ErrorType::InvalidHeaderName));
                            }
                        }

                        field_name = string.into();
                    } else {
                        if matches!(
                            &string,
                            Value::Text(value) if value.len() > self.compiler.max_header_size
                        ) {
                            return Err(self
                                .tokens
                                .unwrap_next()?
                                .custom(ErrorType::HeaderTooLong));
                        }
                        value = string;
                        break;
                    }
                }
            }
        }
        self.instructions.push(Instruction::AddHeader(AddHeader {
            last,
            field_name: field_name.unwrap(),
            value,
        }));
        Ok(())
    }

    pub(crate) fn parse_deleteheader(&mut self) -> Result<(), CompileError> {
        let field_name: Value;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut index = None;
        let mut index_last = false;
        let mut mime = false;
        let mut mime_anychild = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    self.validate_argument(
                        1,
                        match word {
                            Word::Value | Word::Count => Capability::Relational.into(),
                            Word::Regex => Capability::Regex.into(),
                            Word::List => Capability::ExtLists.into(),
                            _ => None,
                        },
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::Index) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    index = (self.tokens.expect_number(u16::MAX as usize)? as i32).into();
                }
                Token::Tag(Word::Last) => {
                    self.validate_argument(4, None, token_info.line_num, token_info.line_pos)?;
                    index_last = true;
                }
                Token::Tag(Word::Mime) => {
                    self.validate_argument(
                        5,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    self.validate_argument(
                        6,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime_anychild = true;
                }
                _ => {
                    field_name = self.parse_string_token(token_info)?;
                    if let Value::Text(header_name) = &field_name {
                        if HeaderName::parse(header_name.as_ref()).is_none() {
                            return Err(self
                                .tokens
                                .unwrap_next()?
                                .custom(ErrorType::InvalidHeaderName));
                        }
                    }
                    break;
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.missing_tag(":mime"));
        }

        let cmd = Instruction::DeleteHeader(DeleteHeader {
            index: if index_last { index.map(|i| -i) } else { index },
            comparator,
            match_type,
            field_name,
            value_patterns: if let Some(Ok(
                Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
            )) = self.tokens.peek().map(|r| r.map(|t| &t.token))
            {
                let mut key_list = self.parse_strings(false)?;
                self.validate_match(&match_type, &mut key_list)?;
                key_list
            } else {
                Vec::new()
            },
            mime_anychild,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
