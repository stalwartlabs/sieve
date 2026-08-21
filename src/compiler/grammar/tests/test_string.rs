/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    CompileError, RawValue, Value,
    grammar::{Capability, Comparator, instruction::CompilerState},
    lexer::{Token, word::Word},
};

use crate::compiler::grammar::{MatchType, test::Test};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestString {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub source: Vec<Value>,
    pub key_list: Vec<Value>,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_string(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut source = None;
        let key_list: Vec<RawValue>;

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
                _ => {
                    if source.is_none() {
                        source = self.parse_strings_token(token_info)?.into();
                    } else {
                        key_list = self.parse_raw_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }
        let key_list = self.validate_match(&match_type, &comparator, key_list)?;

        Ok(Test::String(TestString {
            source: source.unwrap(),
            key_list,
            match_type,
            comparator,
            is_not: false,
        }))
    }
}
