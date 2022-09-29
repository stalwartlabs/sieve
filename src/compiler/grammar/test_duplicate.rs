use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use super::test::Test;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestDuplicate {
    pub handle: Option<StringItem>,
    pub dup_match: DupMatch,
    pub seconds: Option<u64>,
    pub last: bool,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DupMatch {
    Header(StringItem),
    UniqueId(StringItem),
    Default,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_duplicate(&mut self) -> Result<Test, CompileError> {
        let mut handle = None;
        let mut dup_match = DupMatch::Default;
        let mut seconds = None;
        let mut last = false;

        while let Some(token_info) = self.peek() {
            match token_info?.token {
                Token::Tag(Word::Handle) => {
                    self.next();
                    handle = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Header) => {
                    self.next();
                    dup_match = DupMatch::Header(self.unwrap_string()?);
                }
                Token::Tag(Word::UniqueId) => {
                    self.next();
                    dup_match = DupMatch::UniqueId(self.unwrap_string()?);
                }
                Token::Tag(Word::Seconds) => {
                    self.next();
                    seconds = (self.unwrap_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::Last) => {
                    self.next();
                    last = true;
                }
                _ => break,
            }
        }

        Ok(Test::Duplicate(TestDuplicate {
            handle,
            dup_match,
            seconds,
            last,
        }))
    }
}
