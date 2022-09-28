use serde::{Deserialize, Serialize};

use super::{
    lexer::{tokenizer::Tokenizer, Token},
    CompileError,
};

pub mod ast;
pub mod comparator;
pub mod require;
pub mod string_list;
pub mod test;
pub mod test_address;
pub mod test_envelope;
pub mod test_header;
pub mod test_size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AddressPart {
    LocalPart,
    Domain,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MatchType {
    Is,
    Contains,
    Matches,
}

impl<'x> Tokenizer<'x> {
    #[inline(always)]
    pub fn expect_command_end(&mut self) -> Result<(), CompileError> {
        self.expect_token(Token::Semicolon)
    }

    pub fn ignore_command(&mut self) -> Result<(), CompileError> {
        // Skip entire command
        let mut curly_count = 0;
        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Semicolon if curly_count == 0 => {
                    break;
                }
                Token::CurlyOpen => {
                    curly_count += 1;
                }
                Token::CurlyClose => match curly_count {
                    0 => {
                        return Err(token_info.expected("command"));
                    }
                    1 => {
                        break;
                    }
                    _ => curly_count -= 1,
                },
                _ => (),
            }
        }

        Ok(())
    }
}
