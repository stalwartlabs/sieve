use serde::{Deserialize, Serialize};

use crate::{
    compiler::{lexer::tokenizer::Tokenizer, CompileError},
    runtime::StringItem,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestValidExtList {
    pub list_names: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_valid_ext_list(&mut self) -> Result<Test, CompileError> {
        Ok(Test::ValidExtList(TestValidExtList {
            list_names: self.parse_strings(false)?,
        }))
    }
}
