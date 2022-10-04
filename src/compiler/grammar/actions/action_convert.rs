use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{
        command::{Command, CompilerState},
        test::Test,
    },
    lexer::string::StringItem,
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Convert {
    pub from_media_type: StringItem,
    pub to_media_type: StringItem,
    pub transcoding_params: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_convert(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Convert(Convert {
            from_media_type: self.parse_string()?,
            to_media_type: self.parse_string()?,
            transcoding_params: self.parse_strings(false)?,
        }))
    }

    pub(crate) fn parse_convert(&mut self) -> Result<(), CompileError> {
        let cmd = Command::Convert(Convert {
            from_media_type: self.parse_string()?,
            to_media_type: self.parse_string()?,
            transcoding_params: self.parse_strings(false)?,
        });
        self.commands.push(cmd);
        Ok(())
    }
}
