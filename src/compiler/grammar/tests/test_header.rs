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
    grammar::{
        actions::action_mime::MimeOpts,
        instruction::{CompilerState, MapLocalVars},
        Capability, Comparator,
    },
    lexer::{word::Word, Token},
    CompileError, ErrorType, Value,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestHeader {
    pub header_list: Vec<Value>,
    pub key_list: Vec<Value>,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub index: Option<i32>,

    pub mime_opts: MimeOpts<Value>,
    pub mime_anychild: bool,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_header(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_list = None;
        let mut key_list;
        let mut index = None;
        let mut index_last = false;

        let mut mime = false;
        let mut mime_opts = MimeOpts::None;
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
                    | Word::Regex
                    | Word::List),
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
                    self.validate_argument(
                        3,
                        Capability::Index.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;

                    index = (self.tokens.expect_number(u16::MAX as usize)? as i32).into();
                }
                Token::Tag(Word::Last) => {
                    self.validate_argument(
                        4,
                        Capability::Index.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;

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
                Token::Tag(
                    word @ (Word::Type | Word::Subtype | Word::ContentType | Word::Param),
                ) => {
                    self.validate_argument(
                        7,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime_opts = self.parse_mimeopts(word)?;
                }
                _ => {
                    if header_list.is_none() {
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
                        header_list = headers.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }

        if !mime && (mime_anychild || mime_opts != MimeOpts::None) {
            return Err(self.tokens.unwrap_next()?.missing_tag(":mime"));
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::Header(TestHeader {
            header_list: header_list.unwrap(),
            key_list,
            match_type,
            comparator,
            index: if index_last { index.map(|i| -i) } else { index },
            mime_opts,
            mime_anychild,
            is_not: false,
        }))
    }
}

impl MapLocalVars for MimeOpts<Value> {
    fn map_local_vars(&mut self, last_id: usize) {
        if let MimeOpts::Param(value) = self {
            value.map_local_vars(last_id)
        }
    }
}
