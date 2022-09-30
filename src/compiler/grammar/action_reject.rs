use serde::{Deserialize, Serialize};

use crate::{
    compiler::{lexer::tokenizer::Tokenizer, CompileError},
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Reject {
    pub ereject: bool,
    pub reason: StringItem,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_reject(&mut self, ereject: bool) -> Result<Reject, CompileError> {
        Ok(Reject {
            ereject,
            reason: self.unwrap_string()?,
        })
    }
}
