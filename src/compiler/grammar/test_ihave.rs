use serde::{Deserialize, Serialize};

use crate::{
    compiler::{lexer::tokenizer::Tokenizer, CompileError},
    runtime::StringItem,
};

use super::test::Test;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestIhave {
    pub capabilities: Vec<StringItem>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Error {
    pub message: StringItem,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_ihave(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Ihave(TestIhave {
            capabilities: self.parse_strings(false)?,
        }))
    }

    pub(crate) fn parse_error(&mut self) -> Result<Error, CompileError> {
        Ok(Error {
            message: self.unwrap_string()?,
        })
    }
}
