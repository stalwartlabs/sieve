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
    grammar::{instruction::CompilerState, Capability, Comparator},
    lexer::{word::Word, Token},
    CompileError, Value,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestBody {
    pub key_list: Vec<Value>,
    pub body_transform: BodyTransform,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum BodyTransform {
    Raw,
    Content(Vec<Value>),
    Text,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_body(&mut self) -> Result<Test, CompileError> {
        let mut body_transform = BodyTransform::Text;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let key_list;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Raw) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    body_transform = BodyTransform::Raw;
                }
                Token::Tag(Word::Text) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    body_transform = BodyTransform::Text;
                }
                Token::Tag(Word::Content) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    body_transform = BodyTransform::Content(self.parse_strings()?);
                }
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    self.validate_argument(
                        2,
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
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    comparator = self.parse_comparator()?;
                }
                _ => {
                    key_list = self.parse_strings_token(token_info)?;
                    break;
                }
            }
        }
        self.validate_match(&match_type, &key_list)?;

        Ok(Test::Body(TestBody {
            key_list,
            body_transform,
            match_type,
            comparator,
            is_not: false,
        }))
    }
}
