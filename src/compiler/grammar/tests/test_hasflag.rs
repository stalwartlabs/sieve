/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::compiler::{
    grammar::{instruction::CompilerState, Capability, Comparator},
    lexer::{tokenizer::TokenInfo, word::Word, Token},
    CompileError, ErrorType, Value, VariableType,
};

use crate::compiler::grammar::{test::Test, MatchType};

/*
   Usage: hasflag [MATCH-TYPE] [COMPARATOR]
          [<variable-list: string-list>]
          <list-of-flags: string-list>
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
pub(crate) struct TestHasFlag {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub variable_list: Vec<VariableType>,
    pub flags: Vec<Value>,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_hasflag(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut is_local = false;

        let mut maybe_variables;

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
                Token::Tag(Word::Local) => {
                    is_local = true;
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
                            Value::Text(var_name) => {
                                variable_list.push(
                                    self.register_variable(var_name.to_string(), is_local)
                                        .map_err(|error_type| CompileError {
                                            line_num,
                                            line_pos,
                                            error_type,
                                        })?,
                                );
                            }
                            _ => {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .custom(ErrorType::ExpectedConstantString))
                            }
                        }
                    }
                    let mut flags = self.parse_strings(false)?;
                    self.validate_match(&match_type, &mut flags)?;

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
                self.validate_match(&match_type, &mut maybe_variables)?;

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
