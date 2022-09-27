use super::{
    lexer::{tokenizer::Tokenizer, Token},
    CompileError,
};

pub mod ast;
pub mod iff;
pub mod require;

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
                        return Err(token_info.into());
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
