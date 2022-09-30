use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use crate::compiler::grammar::{comparator::Comparator, test::Test, MatchType};

/*
   Usage: hasflag [MATCH-TYPE] [COMPARATOR]
          [<variable-list: string-list>]
          <list-of-flags: string-list>
*/

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestHasFlag {
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub variable_list: Vec<StringItem>,
    pub flags: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_hasflag(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;

        let maybe_flags;

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
                    maybe_flags = vec![string];
                    break;
                }
                Token::BracketOpen => {
                    maybe_flags = self.parse_string_list(false)?;
                    break;
                }
                _ => {
                    return Err(token_info.expected("string or string list"));
                }
            }
        }

        match self.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::String(_) | Token::BracketOpen)) => {
                if !maybe_flags.is_empty() {
                    Ok(Test::HasFlag(TestHasFlag {
                        comparator,
                        match_type,
                        variable_list: maybe_flags,
                        flags: self.parse_strings(match_type == MatchType::Matches)?,
                    }))
                } else {
                    Err(self
                        .unwrap_next()?
                        .invalid("variable name cannot be a list"))
                }
            }
            _ => Ok(Test::HasFlag(TestHasFlag {
                comparator,
                match_type,
                variable_list: Vec::new(),
                flags: if match_type == MatchType::Matches {
                    maybe_flags.into_iter().map(|f| f.into_matches()).collect()
                } else {
                    maybe_flags
                },
            })),
        }
    }
}
