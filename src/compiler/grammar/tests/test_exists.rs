use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::CompilerState,
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestExists {
    pub header_names: Vec<StringItem>,
    pub mime_anychild: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_exists(&mut self) -> Result<Test, CompileError> {
        let mut header_names = None;

        let mut mime = false;
        let mut mime_anychild = false;

        while header_names.is_none() {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    mime_anychild = true;
                }
                _ => {
                    header_names = self.parse_strings_token(token_info)?.into();
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.invalid("missing ':mime' tag"));
        }

        Ok(Test::Exists(TestExists {
            header_names: header_names.unwrap(),
            mime_anychild,
        }))
    }
}
