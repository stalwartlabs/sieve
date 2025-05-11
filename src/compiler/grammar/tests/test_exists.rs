/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::HeaderName;


use crate::compiler::{
    grammar::{instruction::CompilerState, Capability},
    lexer::{word::Word, Token},
    CompileError, ErrorType, Value,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestExists {
    pub header_names: Vec<Value>,
    pub mime_anychild: bool,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_exists(&mut self) -> Result<Test, CompileError> {
        let mut header_names = None;

        let mut mime = false;
        let mut mime_anychild = false;

        while header_names.is_none() {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    self.validate_argument(
                        1,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    self.validate_argument(
                        2,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime_anychild = true;
                }
                _ => {
                    let headers = self.parse_strings_token(token_info)?;
                    for header in &headers {
                        if let Value::Text(header_name) = &header {
                            if HeaderName::parse(header_name.as_ref()).is_none() {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .custom(ErrorType::InvalidHeaderName));
                            }
                        }
                    }
                    header_names = headers.into();
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.missing_tag(":mime"));
        }

        Ok(Test::Exists(TestExists {
            header_names: header_names.unwrap(),
            mime_anychild,
            is_not: false,
        }))
    }
}
