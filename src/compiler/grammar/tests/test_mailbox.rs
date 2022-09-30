use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use crate::compiler::grammar::{comparator::Comparator, test::Test, MatchType};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMailboxExists {
    pub mailbox_names: Vec<StringItem>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMetadataExists {
    pub mailbox: StringItem,
    pub annotation_names: Vec<StringItem>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestServerMetadataExists {
    pub annotation_names: Vec<StringItem>,
}

/*

metadata [MATCH-TYPE] [COMPARATOR]
           <mailbox: string>
           <annotation-name: string> <key-list: string-list>

*/

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestMetadata {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub mailbox: StringItem,
    pub annotation_name: StringItem,
    pub key_list: Vec<StringItem>,
}

/*

servermetadata [MATCH-TYPE] [COMPARATOR]
           <annotation-name: string> <key-list: string-list>

*/

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestServerMetadata {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub annotation_name: StringItem,
    pub key_list: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_mailboxexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MailboxExists(TestMailboxExists {
            mailbox_names: self.parse_strings(false)?,
        }))
    }

    pub(crate) fn parse_test_metadataexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::MetadataExists(TestMetadataExists {
            mailbox: self.unwrap_string()?,
            annotation_names: self.parse_strings(false)?,
        }))
    }

    pub(crate) fn parse_test_servermetadataexists(&mut self) -> Result<Test, CompileError> {
        Ok(Test::ServerMetadataExists(TestServerMetadataExists {
            annotation_names: self.parse_strings(false)?,
        }))
    }

    pub(crate) fn parse_test_metadata(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut mailbox = None;
        let mut annotation_name = None;
        let key_list: Vec<StringItem>;

        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::String(string) => {
                    if mailbox.is_none() {
                        mailbox = string.into();
                    } else if annotation_name.is_none() {
                        annotation_name = string.into();
                    } else {
                        key_list = vec![if match_type == MatchType::Matches {
                            string.into_matches()
                        } else {
                            string
                        }];
                        break;
                    }
                }
                Token::BracketOpen => {
                    if mailbox.is_some() && annotation_name.is_some() {
                        key_list = self.parse_string_list(match_type == MatchType::Matches)?;
                        break;
                    } else {
                        return Err(token_info.expected("string"));
                    }
                }
                _ => {
                    return Err(token_info.expected(
                        if mailbox.is_none() || annotation_name.is_none() {
                            "string"
                        } else {
                            "string or string list"
                        },
                    ));
                }
            }
        }

        Ok(Test::Metadata(TestMetadata {
            match_type,
            comparator,
            mailbox: mailbox.unwrap(),
            annotation_name: annotation_name.unwrap(),
            key_list,
        }))
    }

    pub(crate) fn parse_test_servermetadata(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut annotation_name = None;
        let key_list: Vec<StringItem>;

        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::String(string) => {
                    if annotation_name.is_none() {
                        annotation_name = string.into();
                    } else {
                        key_list = vec![if match_type == MatchType::Matches {
                            string.into_matches()
                        } else {
                            string
                        }];
                        break;
                    }
                }
                Token::BracketOpen => {
                    if annotation_name.is_some() {
                        key_list = self.parse_string_list(match_type == MatchType::Matches)?;
                        break;
                    } else {
                        return Err(token_info.expected("string"));
                    }
                }
                _ => {
                    return Err(token_info.expected(if annotation_name.is_none() {
                        "string"
                    } else {
                        "string or string list"
                    }));
                }
            }
        }

        Ok(Test::ServerMetadata(TestServerMetadata {
            match_type,
            comparator,
            annotation_name: annotation_name.unwrap(),
            key_list,
        }))
    }
}
