use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, Token},
        CompileError,
    },
    runtime::StringItem,
};

use super::capability::Capability;

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_require(
        &mut self,
        capabilities: &mut Vec<Capability>,
    ) -> Result<(), CompileError> {
        let token_info = self.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => loop {
                let token_info = self.unwrap_next()?;
                match token_info.token {
                    Token::String(StringItem::Text(value)) => {
                        capabilities.push(Capability::parse(value));
                        let token_info = self.unwrap_next()?;
                        match token_info.token {
                            Token::Comma => (),
                            Token::BracketClose => break,
                            _ => {
                                return Err(token_info.expected("']' or ','"));
                            }
                        }
                    }
                    _ => {
                        return Err(token_info.expected("string"));
                    }
                }
            },
            Token::String(StringItem::Text(value)) => {
                capabilities.push(Capability::parse(value));
            }
            _ => {
                return Err(token_info.expected("'[' or string"));
            }
        }

        Ok(())
    }
}
