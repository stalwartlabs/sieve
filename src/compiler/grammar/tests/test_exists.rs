use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestExists {
    pub header_names: Vec<StringItem>,
    pub mime: bool,
    pub mime_anychild: bool,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_exists(&mut self) -> Result<Test, CompileError> {
        let mut header_names = None;

        let mut mime = false;
        let mut mime_anychild = false;

        while header_names.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    mime_anychild = true;
                }
                Token::String(string) => {
                    header_names = vec![string].into();
                }
                Token::BracketOpen => {
                    header_names = self.parse_string_list(false)?.into();
                }
                _ => {
                    return Err(token_info.expected("string or string list"));
                }
            }
        }

        Ok(Test::Exists(TestExists {
            header_names: header_names.unwrap(),
            mime,
            mime_anychild,
        }))
    }
}
