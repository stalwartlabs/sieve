use crate::compiler::{lexer::tokenizer::Tokenizer, CompileError};

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_if(&mut self) -> Result<(), CompileError> {
        Ok(())
    }
}
