use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

/*

include [LOCATION] [":once"] [":optional"] <value: string>
  LOCATION = ":personal" / ":global"

*/

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Include {
    pub location: Location,
    pub once: bool,
    pub optional: bool,
    pub value: StringItem,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Location {
    Personal,
    Global,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Global {
    pub value: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_include(&mut self) -> Result<Include, CompileError> {
        let value;
        let mut once = false;
        let mut optional = false;
        let mut location = Location::Personal;

        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Once) => {
                    once = true;
                }
                Token::Tag(Word::Optional) => {
                    optional = true;
                }
                Token::Tag(Word::Personal) => {
                    location = Location::Personal;
                }
                Token::Tag(Word::Global) => {
                    location = Location::Global;
                }
                Token::String(string) => {
                    value = string;
                    break;
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(Include {
            location,
            once,
            optional,
            value,
        })
    }

    pub(crate) fn parse_global(&mut self) -> Result<Global, CompileError> {
        Ok(Global {
            value: self.parse_strings(false)?,
        })
    }
}
