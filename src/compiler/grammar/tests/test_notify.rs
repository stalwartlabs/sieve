use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{command::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestNotifyMethodCapability {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub notification_uri: StringItem,
    pub notification_capability: StringItem,
    pub key_list: Vec<StringItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestValidNotifyMethod {
    pub notification_uris: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
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
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                _ => {
                    if notification_uri.is_none() {
                        notification_uri = self.parse_string_token(token_info)?.into();
                    } else if notification_capability.is_none() {
                        notification_capability = self.parse_string_token(token_info)?.into();
                    } else {
                        key_list = self.parse_strings_token(token_info, match_type.is_matches())?;
                        break;
                    }
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
