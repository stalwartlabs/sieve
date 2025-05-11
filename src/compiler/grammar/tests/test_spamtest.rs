/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::compiler::{
    grammar::{instruction::CompilerState, Capability, Comparator},
    lexer::{word::Word, Token},
    CompileError, Value,
};

use crate::compiler::grammar::{test::Test, MatchType};

/*
           Usage:    spamtest [":percent"] [COMPARATOR] [MATCH-TYPE]
                     <value: string>
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
pub(crate) struct TestSpamTest {
    pub value: Value,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub percent: bool,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestVirusTest {
    pub value: Value,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_spamtest(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut percent = false;
        let value;

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
                Token::Tag(Word::Percent) => {
                    self.validate_argument(
                        3,
                        Capability::SpamTestPlus.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    percent = true;
                }
                _ => {
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        Ok(Test::SpamTest(TestSpamTest {
            value,
            percent,
            match_type,
            comparator,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_virustest(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let value;

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
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        Ok(Test::VirusTest(TestVirusTest {
            value,
            match_type,
            comparator,
            is_not: false,
        }))
    }
}
