use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use super::{comparator::Comparator, test::Test, MatchType};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestBody {
    pub key_list: Vec<StringItem>,
    pub body_transform: BodyTransform,
    pub match_type: MatchType,
    pub comparator: Comparator,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum BodyTransform {
    Raw,
    Content(Vec<StringItem>),
    Text,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_body(&mut self) -> Result<Test, CompileError> {
        let mut body_transform = BodyTransform::Text;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut key_list = None;

        while key_list.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Raw) => body_transform = BodyTransform::Raw,
                Token::Tag(Word::Text) => body_transform = BodyTransform::Text,
                Token::Tag(Word::Content) => {
                    body_transform = BodyTransform::Content(self.parse_strings(false)?);
                }
                Token::Tag(
                    word @ (Word::Is | Word::Contains | Word::Matches | Word::Value | Word::Count),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::String(string) => {
                    key_list = vec![if match_type == MatchType::Matches {
                        string.into_matches()
                    } else {
                        string
                    }]
                    .into();
                }
                Token::BracketOpen => {
                    key_list = self
                        .parse_string_list(match_type == MatchType::Matches)?
                        .into();
                }
                _ => {
                    return Err(token_info.expected("string or string list"));
                }
            }
        }

        Ok(Test::Body(TestBody {
            key_list: key_list.unwrap(),
            body_transform,
            match_type,
            comparator,
        }))
    }
}
