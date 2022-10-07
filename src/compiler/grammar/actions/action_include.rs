use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

/*

include [LOCATION] [":once"] [":optional"] <value: string>
  LOCATION = ":personal" / ":global"

*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Include {
    pub location: Location,
    pub once: bool,
    pub optional: bool,
    pub value: StringItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Location {
    Personal,
    Global,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_include(&mut self) -> Result<(), CompileError> {
        let value;
        let mut once = false;
        let mut optional = false;
        let mut location = Location::Personal;

        loop {
            let token_info = self.tokens.unwrap_next()?;
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
                _ => {
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.instructions.push(Instruction::Include(Include {
            location,
            once,
            optional,
            value,
        }));
        Ok(())
    }
}
