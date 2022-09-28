use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
    Comparator,
};

use super::{test::Test, MatchType};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestHeader {
    pub header_list: Vec<StringItem>,
    pub key_list: Vec<StringItem>,
    pub match_type: MatchType,
    pub comparator: Comparator,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_header(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_list = None;
        let mut key_list = None;

        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(word @ (Word::Is | Word::Contains | Word::Matches)) => {
                    match_type = word.into();
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::String(string) => {
                    if header_list.is_none() {
                        header_list = vec![string].into();
                    } else if key_list.is_none() {
                        key_list = vec![if match_type == MatchType::Matches {
                            string.into_matches()
                        } else {
                            string
                        }]
                        .into();
                        break;
                    }
                }
                Token::BracketOpen => {
                    if header_list.is_none() {
                        header_list = self.parse_string_list(false)?.into();
                    } else if key_list.is_none() {
                        key_list = self
                            .parse_string_list(match_type == MatchType::Matches)?
                            .into();
                        break;
                    }
                }
                _ => {
                    return Err(token_info.expected("string or string list"));
                }
            }
        }

        Ok(Test::Header(TestHeader {
            header_list: header_list.unwrap(),
            key_list: key_list.unwrap(),
            match_type,
            comparator,
        }))
    }
}
