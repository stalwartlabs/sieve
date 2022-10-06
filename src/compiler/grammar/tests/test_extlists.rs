use serde::{Deserialize, Serialize};

use crate::compiler::grammar::command::CompilerState;
use crate::compiler::lexer::string::StringItem;
use crate::compiler::CompileError;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestValidExtList {
    pub list_names: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_valid_ext_list(&mut self) -> Result<Test, CompileError> {
        Ok(Test::ValidExtList(TestValidExtList {
            list_names: self.parse_strings()?,
        }))
    }
}
