use mail_parser::HeaderName;
use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::CompilerState,
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
    pub is_not: bool,
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
            let token_info = token_info?;
            let line_num = token_info.line_num;
            let line_pos = token_info.line_pos;

            match token_info.token {
                Token::Tag(Word::Handle) => {
                    self.validate_argument(1, None, line_num, line_pos)?;
                    self.tokens.next();
                    handle = self.parse_string()?.into();
                }
                Token::Tag(Word::Header) => {
                    self.validate_argument(2, None, line_num, line_pos)?;
                    self.tokens.next();
                    let header = self.parse_string()?;
                    if let StringItem::Text(header_name) = &header {
                        if HeaderName::parse(header_name).is_none() {
                            return Err(self.tokens.unwrap_next()?.invalid("invalid header name"));
                        }
                    }
                    dup_match = DupMatch::Header(header);
                }
                Token::Tag(Word::UniqueId) => {
                    self.validate_argument(2, None, line_num, line_pos)?;
                    self.tokens.next();
                    dup_match = DupMatch::UniqueId(self.parse_string()?);
                }
                Token::Tag(Word::Seconds) => {
                    self.validate_argument(3, None, line_num, line_pos)?;
                    self.tokens.next();
                    seconds = (self.tokens.expect_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::Last) => {
                    self.validate_argument(4, None, line_num, line_pos)?;
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
            is_not: false,
        }))
    }
}
