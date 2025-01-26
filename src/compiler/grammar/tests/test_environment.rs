/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    grammar::{instruction::CompilerState, Capability, Comparator},
    lexer::{word::Word, Token},
    CompileError, Value, VariableType,
};

use crate::compiler::grammar::{test::Test, MatchType};

use super::test_string::TestString;

impl CompilerState<'_> {
    pub(crate) fn parse_test_environment(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut name = None;
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
                    if name.is_none() {
                        if let Token::StringConstant(s) = token_info.token {
                            name = Value::Variable(VariableType::Environment(
                                s.into_string().to_lowercase(),
                            ))
                            .into();
                        } else {
                            return Err(token_info.expected("environment variable"));
                        }
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::Environment(TestString {
            source: vec![name.unwrap()],
            key_list,
            match_type,
            comparator,
            is_not: false,
        }))
    }
}
