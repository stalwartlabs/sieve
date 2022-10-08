use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::CompilerState,
    lexer::{word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestSize {
    pub over: bool,
    pub limit: usize,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_size(&mut self) -> Result<Test, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        let over = match token_info.token {
            Token::Tag(Word::Over) => true,
            Token::Tag(Word::Under) => false,
            _ => {
                return Err(token_info.expected("':over' or ':under'"));
            }
        };
        let token_info = self.tokens.unwrap_next()?;
        if let Token::Number(limit) = token_info.token {
            Ok(Test::Size(TestSize {
                over,
                limit,
                is_not: false,
            }))
        } else {
            Err(token_info.expected("number"))
        }
    }
}
