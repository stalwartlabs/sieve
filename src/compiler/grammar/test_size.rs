use serde::{Deserialize, Serialize};

use crate::compiler::{
    lexer::{tokenizer::Tokenizer, word::Word, Token},
    CompileError,
};

use super::test::Test;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestSize {
    pub over: bool,
    pub limit: usize,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_size(&mut self) -> Result<Test, CompileError> {
        let token_info = self.unwrap_next()?;
        let over = match token_info.token {
            Token::Tag(Word::Over) => true,
            Token::Tag(Word::Under) => false,
            _ => {
                return Err(token_info.expected("':over' or ':under'"));
            }
        };
        let token_info = self.unwrap_next()?;
        if let Token::Number(limit) = token_info.token {
            Ok(Test::Size(TestSize { over, limit }))
        } else {
            Err(token_info.expected("number"))
        }
    }
}
