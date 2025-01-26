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

use mail_parser::HeaderName;
use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{instruction::CompilerState, Capability},
    lexer::{word::Word, Token},
    CompileError, ErrorType, Value,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
