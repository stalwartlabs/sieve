/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
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
    grammar::{instruction::CompilerState, test::Test, Capability, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{AddressPart, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestAddress {
    pub header_list: Vec<StringItem>,
    pub key_list: Vec<StringItem>,
    pub address_part: AddressPart,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub index: Option<i32>,

    pub mime_anychild: bool,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_address(&mut self) -> Result<Test, CompileError> {
        let mut address_part = AddressPart::All;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_list = None;
        let key_list;
        let mut index = None;
        let mut index_last = false;

        let mut mime = false;
        let mut mime_anychild = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::LocalPart | Word::Domain | Word::All | Word::User | Word::Detail),
                ) => {
                    self.validate_argument(
                        1,
                        if matches!(word, Word::User | Word::Detail) {
                            Capability::SubAddress.into()
                        } else {
                            None
                        },
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    address_part = word.into();
                }
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
                Token::Tag(Word::Index) => {
                    self.validate_argument(
                        4,
                        Capability::Index.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    index = (self.tokens.expect_number(u16::MAX as usize)? as i32).into();
                }
                Token::Tag(Word::Last) => {
                    self.validate_argument(
                        5,
                        Capability::Index.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    index_last = true;
                }
                Token::Tag(Word::Mime) => {
                    self.validate_argument(
                        6,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    self.validate_argument(
                        7,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime_anychild = true;
                }
                _ => {
                    if header_list.is_none() {
                        header_list = self.parse_strings_token(token_info)?.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.missing_tag(":mime"));
        }
        self.validate_match(&match_type, &key_list)?;

        Ok(Test::Address(TestAddress {
            header_list: header_list.unwrap(),
            key_list,
            address_part,
            match_type,
            comparator,
            index: if index_last { index.map(|i| -i) } else { index },
            mime_anychild,
            is_not: false,
        }))
    }
}

impl From<Word> for AddressPart {
    fn from(word: Word) -> Self {
        match word {
            Word::LocalPart => AddressPart::LocalPart,
            Word::Domain => AddressPart::Domain,
            Word::All => AddressPart::All,
            Word::User => AddressPart::User,
            Word::Detail => AddressPart::Detail,
            _ => unreachable!(),
        }
    }
}
