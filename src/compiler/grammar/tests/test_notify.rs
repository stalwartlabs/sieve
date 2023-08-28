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
pub(crate) struct TestNotifyMethodCapability {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub notification_uri: Value,
    pub notification_capability: Value,
    pub key_list: Vec<Value>,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestValidNotifyMethod {
    pub notification_uris: Vec<Value>,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_valid_notify_method(&mut self) -> Result<Test, CompileError> {
        Ok(Test::ValidNotifyMethod(TestValidNotifyMethod {
            notification_uris: self.parse_strings(false)?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_notify_method_capability(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut notification_uri = None;
        let mut notification_capability = None;
        let mut key_list;

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
                _ => {
                    if notification_uri.is_none() {
                        notification_uri = self.parse_string_token(token_info)?.into();
                    } else if notification_capability.is_none() {
                        notification_capability = self.parse_string_token(token_info)?.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::NotifyMethodCapability(TestNotifyMethodCapability {
            key_list,
            match_type,
            comparator,
            notification_uri: notification_uri.unwrap(),
            notification_capability: notification_capability.unwrap(),
            is_not: false,
        }))
    }
}
