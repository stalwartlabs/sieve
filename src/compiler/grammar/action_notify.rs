use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

/*
notify [":from" string]
           [":importance" <"1" / "2" / "3">]
           [":options" string-list]
           [":message" string]
           <method: string>

*/

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Notify {
    pub from: Option<StringItem>,
    pub importance: Option<StringItem>,
    pub options: Vec<StringItem>,
    pub message: Option<StringItem>,
    pub method: StringItem,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_notify(&mut self) -> Result<Notify, CompileError> {
        let mut method = None;
        let mut from = None;
        let mut importance = None;
        let mut message = None;
        let mut options = Vec::new();

        while method.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::From) => {
                    from = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Message) => {
                    message = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Importance) => {
                    importance = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Options) => {
                    options = self.parse_strings(false)?;
                }
                Token::String(string) => {
                    method = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(Notify {
            method: method.unwrap(),
            from,
            importance,
            options,
            message,
        })
    }
}
