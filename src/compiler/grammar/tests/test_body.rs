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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestBody {
    pub key_list: Vec<Value>,
    pub body_transform: BodyTransform,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub include_subject: bool,
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
pub(crate) enum BodyTransform {
    Raw,
    Content(Vec<Value>),
    Text,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_body(&mut self) -> Result<Test, CompileError> {
        let mut body_transform = BodyTransform::Text;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut key_list;
        let mut include_subject = false;

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
                    body_transform = BodyTransform::Content(self.parse_strings(false)?);
                }
                Token::Tag(Word::Subject) => {
                    self.validate_argument(4, None, token_info.line_num, token_info.line_pos)?;
                    include_subject = true;
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
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::Body(TestBody {
            key_list,
            body_transform,
            match_type,
            comparator,
            include_subject,
            is_not: false,
        }))
    }
}
