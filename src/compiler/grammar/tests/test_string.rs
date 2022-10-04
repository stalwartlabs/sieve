use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{command::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestString {
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub source: Vec<StringItem>,
    pub key_list: Vec<StringItem>,

    pub list: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_string(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut source = None;
        let key_list: Vec<StringItem>;

        let mut list = false;

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
                Token::Tag(Word::List) => {
                    list = true;
                }
                _ => {
                    if source.is_none() {
                        source = self.parse_strings_token(token_info, false)?.into();
                    } else {
                        key_list =
                            self.parse_strings_token(token_info, match_type == MatchType::Matches)?;
                        break;
                    }
                }
            }
        }

        Ok(Test::String(TestString {
            source: source.unwrap(),
            key_list,
            match_type,
            comparator,
            list,
        }))
    }
}
