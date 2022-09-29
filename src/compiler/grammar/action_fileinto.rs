use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileInto {
    pub copy: bool,
    pub create: bool,
    pub folder: StringItem,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_fileinto(&mut self) -> Result<FileInto, CompileError> {
        let mut folder = None;
        let mut copy = false;
        let mut create = false;

        while folder.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Copy) => {
                    copy = true;
                }
                Token::Tag(Word::Create) => {
                    create = true;
                }
                Token::String(string) => {
                    folder = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(FileInto {
            folder: folder.unwrap(),
            copy,
            create,
        })
    }
}
