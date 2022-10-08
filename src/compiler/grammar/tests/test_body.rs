use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{instruction::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestBody {
    pub key_list: Vec<StringItem>,
    pub body_transform: BodyTransform,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum BodyTransform {
    Raw,
    Content(Vec<StringItem>),
    Text,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_body(&mut self) -> Result<Test, CompileError> {
        let mut body_transform = BodyTransform::Text;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let key_list;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Raw) => body_transform = BodyTransform::Raw,
                Token::Tag(Word::Text) => body_transform = BodyTransform::Text,
                Token::Tag(Word::Content) => {
                    body_transform = BodyTransform::Content(self.parse_strings()?);
                }
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
                    key_list = self.parse_strings_token(token_info)?;
                    break;
                }
            }
        }

        Ok(Test::Body(TestBody {
            key_list,
            body_transform,
            match_type,
            comparator,
            is_not: false,
        }))
    }
}
