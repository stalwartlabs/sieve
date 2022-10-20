use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        grammar::{instruction::CompilerState, Capability, Comparator},
        lexer::{string::StringItem, word::Word, Token},
        CompileError,
    },
    Metadata,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMailboxExists {
    pub mailbox_names: Vec<StringItem>,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMetadataExists {
    pub mailbox: Option<StringItem>,
    pub annotation_names: Vec<StringItem>,
    pub is_not: bool,
}

/*

metadata [MATCH-TYPE] [COMPARATOR]
           <mailbox: string>
           <annotation-name: string> <key-list: string-list>

*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMetadata {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub medatata: Metadata<StringItem>,
    pub key_list: Vec<StringItem>,
    pub is_not: bool,
}

/*

servermetadata [MATCH-TYPE] [COMPARATOR]
           <annotation-name: string> <key-list: string-list>

*/

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_mailboxexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MailboxExists(TestMailboxExists {
            mailbox_names: self.parse_strings()?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_metadataexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MetadataExists(TestMetadataExists {
            mailbox: self.parse_string()?.into(),
            annotation_names: self.parse_strings()?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_servermetadataexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MetadataExists(TestMetadataExists {
            mailbox: None,
            annotation_names: self.parse_strings()?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_metadata(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut mailbox = None;
        let mut annotation_name = None;
        let key_list: Vec<StringItem>;

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
        self.validate_match(&match_type, &key_list)?;

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
        let key_list: Vec<StringItem>;

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
        self.validate_match(&match_type, &key_list)?;

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
