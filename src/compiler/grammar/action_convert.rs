use serde::{Deserialize, Serialize};

use crate::{
    compiler::{lexer::tokenizer::Tokenizer, CompileError},
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Convert {
    pub from_media_type: StringItem,
    pub to_media_type: StringItem,
    pub transcoding_params: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_convert(&mut self) -> Result<Convert, CompileError> {
        Ok(Convert {
            from_media_type: self.unwrap_string()?,
            to_media_type: self.unwrap_string()?,
            transcoding_params: self.parse_strings(false)?,
        })
    }
}
