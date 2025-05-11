/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::{
    compiler::{
        grammar::{
            instruction::{CompilerState, MapLocalVars},
            Capability, Comparator,
        },
        lexer::{word::Word, Token},
        CompileError, Value,
    },
    Metadata,
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
pub(crate) struct TestMailboxExists {
    pub mailbox_names: Vec<Value>,
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
pub(crate) struct TestMetadataExists {
    pub mailbox: Option<Value>,
    pub annotation_names: Vec<Value>,
    pub is_not: bool,
}

/*

metadata [MATCH-TYPE] [COMPARATOR]
           <mailbox: string>
           <annotation-name: string> <key-list: string-list>

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
pub(crate) struct TestMetadata {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub medatata: Metadata<Value>,
    pub key_list: Vec<Value>,
    pub is_not: bool,
}

/*

servermetadata [MATCH-TYPE] [COMPARATOR]
           <annotation-name: string> <key-list: string-list>

*/

impl CompilerState<'_> {
    pub(crate) fn parse_test_mailboxexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MailboxExists(TestMailboxExists {
            mailbox_names: self.parse_strings(false)?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_metadataexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MetadataExists(TestMetadataExists {
            mailbox: self.parse_string()?.into(),
            annotation_names: self.parse_strings(false)?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_servermetadataexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MetadataExists(TestMetadataExists {
            mailbox: None,
            annotation_names: self.parse_strings(false)?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_metadata(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut mailbox = None;
        let mut annotation_name = None;
        let mut key_list: Vec<Value>;

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
                    if mailbox.is_none() {
                        mailbox = self.parse_string_token(token_info)?.into();
                    } else if annotation_name.is_none() {
                        annotation_name = self.parse_string_token(token_info)?.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::Metadata(TestMetadata {
            match_type,
            comparator,
            medatata: Metadata::Mailbox {
                name: mailbox.unwrap(),
                annotation: annotation_name.unwrap(),
            },
            key_list,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_servermetadata(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut annotation_name = None;
        let mut key_list: Vec<Value>;

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
                    if annotation_name.is_none() {
                        annotation_name = self.parse_string_token(token_info)?.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::Metadata(TestMetadata {
            match_type,
            comparator,
            medatata: Metadata::Server {
                annotation: annotation_name.unwrap(),
            },
            key_list,
            is_not: false,
        }))
    }
}

impl MapLocalVars for Metadata<Value> {
    fn map_local_vars(&mut self, last_id: usize) {
        match self {
            Metadata::Mailbox { name, annotation } => {
                name.map_local_vars(last_id);
                annotation.map_local_vars(last_id);
            }
            Metadata::Server { annotation } => {
                annotation.map_local_vars(last_id);
            }
        }
    }
}
