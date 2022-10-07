use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{instruction::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

/*
   Usage: hasflag [MATCH-TYPE] [COMPARATOR]
          [<variable-list: string-list>]
          <list-of-flags: string-list>
*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestHasFlag {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub variable_list: Vec<StringItem>,
    pub flags: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_hasflag(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;

        let maybe_flags;

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
                    maybe_flags = self.parse_strings_token(token_info)?;
                    break;
                }
            }
        }

        match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen)) => {
                if !maybe_flags.is_empty() {
                    Ok(Test::HasFlag(TestHasFlag {
                        comparator,
                        match_type,
                        variable_list: maybe_flags,
                        flags: self.parse_strings()?,
                    }))
                } else {
                    Err(self
                        .tokens
                        .unwrap_next()?
                        .invalid("variable name cannot be a list"))
                }
            }
            _ => Ok(Test::HasFlag(TestHasFlag {
                comparator,
                match_type,
                variable_list: Vec::new(),
                flags: maybe_flags,
            })),
        }
    }
}
