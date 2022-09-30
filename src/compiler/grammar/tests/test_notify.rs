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
pub(crate) struct TestNotifyMethodCapability {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub notification_uri: StringItem,
    pub notification_capability: StringItem,
    pub key_list: Vec<StringItem>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestValidNotifyMethod {
    pub notification_uris: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_valid_notify_method(&mut self) -> Result<Test, CompileError> {
        Ok(Test::ValidNotifyMethod(TestValidNotifyMethod {
            notification_uris: self.parse_strings(false)?,
        }))
    }

    pub(crate) fn parse_test_notify_method_capability(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut notification_uri = None;
        let mut notification_capability = None;
        let key_list;

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
                    if notification_uri.is_none() {
                        notification_uri = string.into();
                    } else if notification_capability.is_none() {
                        notification_capability = string.into();
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
                    if notification_uri.is_some() && notification_capability.is_some() {
                        key_list = self.parse_string_list(match_type == MatchType::Matches)?;
                        break;
                    } else {
                        return Err(token_info.expected("string or string list"));
                    }
                }
                _ => {
                    return Err(token_info.expected(
                        if notification_uri.is_some() && notification_capability.is_some() {
                            "string or string list"
                        } else {
                            "string"
                        },
                    ));
                }
            }
        }

        Ok(Test::NotifyMethodCapability(TestNotifyMethodCapability {
            key_list,
            match_type,
            comparator,
            notification_uri: notification_uri.unwrap(),
            notification_capability: notification_capability.unwrap(),
        }))
    }
}
