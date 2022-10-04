use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::CompilerState,
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestDuplicate {
    pub handle: Option<StringItem>,
    pub dup_match: DupMatch,
    pub seconds: Option<u64>,
    pub last: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum DupMatch {
    Header(StringItem),
    UniqueId(StringItem),
    Default,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_duplicate(&mut self) -> Result<Test, CompileError> {
        let mut handle = None;
        let mut dup_match = DupMatch::Default;
        let mut seconds = None;
        let mut last = false;

        while let Some(token_info) = self.tokens.peek() {
            match token_info?.token {
                Token::Tag(Word::Handle) => {
                    self.tokens.next();
                    handle = self.parse_string()?.into();
                }
                Token::Tag(Word::Header) => {
                    self.tokens.next();
                    dup_match = DupMatch::Header(self.parse_string()?);
                }
                Token::Tag(Word::UniqueId) => {
                    self.tokens.next();
                    dup_match = DupMatch::UniqueId(self.parse_string()?);
                }
                Token::Tag(Word::Seconds) => {
                    self.tokens.next();
                    seconds = (self.tokens.expect_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::Last) => {
                    self.tokens.next();
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
