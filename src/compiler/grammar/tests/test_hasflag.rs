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
    grammar::{actions::action_set::Variable, instruction::CompilerState, Capability, Comparator},
    lexer::{string::StringItem, tokenizer::TokenInfo, word::Word, Token},
    CompileError, ErrorType,
};

use crate::compiler::grammar::{test::Test, MatchType};

/*
   Usage: hasflag [MATCH-TYPE] [COMPARATOR]
          [<variable-list: string-list>]
          <list-of-flags: string-list>
*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestHasFlag {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub variable_list: Vec<Variable>,
    pub flags: Vec<StringItem>,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_hasflag(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;

        let maybe_variables;

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
                    maybe_variables = self.parse_strings_token(token_info)?;
                    break;
                }
            }
        }

        match self.tokens.peek() {
            Some(Ok(TokenInfo {
                token: Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
                line_num,
                line_pos,
            })) => {
                if !maybe_variables.is_empty() {
                    let line_num = *line_num;
                    let line_pos = *line_pos;

                    let mut variable_list = Vec::with_capacity(maybe_variables.len());
                    for variable in maybe_variables {
                        match variable {
                            StringItem::Text(var_name) => {
                                variable_list.push(self.register_variable(var_name).map_err(
                                    |error_type| CompileError {
                                        line_num,
                                        line_pos,
                                        error_type,
                                    },
                                )?);
                            }
                            _ => {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .custom(ErrorType::ExpectedConstantString))
                            }
                        }
                    }
                    let flags = self.parse_strings()?;
                    self.validate_match(&match_type, &flags)?;

                    Ok(Test::HasFlag(TestHasFlag {
                        comparator,
                        match_type,
                        variable_list,
                        flags,
                        is_not: false,
                    }))
                } else {
                    Err(self
                        .tokens
                        .unwrap_next()?
                        .custom(ErrorType::ExpectedConstantString))
                }
            }
            _ => {
                self.validate_match(&match_type, &maybe_variables)?;

                Ok(Test::HasFlag(TestHasFlag {
                    comparator,
                    match_type,
                    variable_list: Vec::new(),
                    flags: maybe_variables,
                    is_not: false,
                }))
            }
        }
    }
}
