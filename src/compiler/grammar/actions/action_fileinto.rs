use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct FileInto {
    pub copy: bool,
    pub create: bool,
    pub folder: StringItem,
    pub flags: Vec<StringItem>,
    pub mailbox_id: Option<StringItem>,
    pub special_use: Option<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_fileinto(&mut self) -> Result<(), CompileError> {
        let folder;
        let mut copy = false;
        let mut create = false;
        let mut flags = Vec::new();
        let mut mailbox_id = None;
        let mut special_use = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Copy) => {
                    copy = true;
                }
                Token::Tag(Word::Create) => {
                    create = true;
                }
                Token::Tag(Word::Flags) => {
                    flags = self.parse_strings()?;
                }
                Token::Tag(Word::MailboxId) => {
                    mailbox_id = self.parse_string()?.into();
                }
                Token::Tag(Word::SpecialUse) => {
                    special_use = self.parse_string()?.into();
                }
                _ => {
                    folder = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.instructions.push(Instruction::FileInto(FileInto {
            folder,
            copy,
            create,
            flags,
            mailbox_id,
            special_use,
        }));
        Ok(())
    }
}
