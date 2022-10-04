use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{command::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

/*
           Usage:    spamtest [":percent"] [COMPARATOR] [MATCH-TYPE]
                     <value: string>
*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestSpamTest {
    pub value: StringItem,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub percent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestVirusTest {
    pub value: StringItem,
    pub match_type: MatchType,
    pub comparator: Comparator,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_spamtest(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut percent = false;
        let value;

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
                Token::Tag(Word::Percent) => {
                    percent = true;
                }
                _ => {
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        Ok(Test::SpamTest(TestSpamTest {
            value,
            percent,
            match_type,
            comparator,
        }))
    }

    pub(crate) fn parse_test_virustest(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let value;

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
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        Ok(Test::VirusTest(TestVirusTest {
            value,
            match_type,
            comparator,
        }))
    }
}
