use mail_parser::HeaderName;
use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{instruction::CompilerState, Capability},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestExists {
    pub header_names: Vec<StringItem>,
    pub mime_anychild: bool,
    pub is_not: bool,
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
                    self.validate_argument(
                        1,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    self.validate_argument(
                        2,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime_anychild = true;
                }
                _ => {
                    let headers = self.parse_strings_token(token_info)?;
                    for header in &headers {
                        if let StringItem::Text(header_name) = &header {
                            if HeaderName::parse(header_name).is_none() {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .invalid("invalid header name"));
                            }
                        }
                    }
                    header_names = headers.into();
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.invalid("missing ':mime' tag"));
        }

        Ok(Test::Exists(TestExists {
            header_names: header_names.unwrap(),
            mime_anychild,
            is_not: false,
        }))
    }
}
