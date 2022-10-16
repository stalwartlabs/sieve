use crate::{
    compiler::{
        grammar::{instruction::CompilerState, Comparator},
        lexer::{string::StringItem, word::Word, Token},
        CompileError,
    },
    runtime::string::IntoString,
};

use crate::compiler::grammar::{test::Test, MatchType};

use super::test_string::TestString;

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_environment(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut name = None;
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
                    if name.is_none() {
                        if let Token::StringConstant(s) = token_info.token {
                            name = StringItem::EnvironmentVariable(s.into_string().to_lowercase())
                                .into();
                        } else {
                            return Err(token_info.expected("environment variable"));
                        }
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }

        Ok(Test::Environment(TestString {
            source: vec![name.unwrap()],
            key_list,
            match_type,
            comparator,
            is_not: false,
        }))
    }
}
