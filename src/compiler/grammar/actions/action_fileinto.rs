/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
        Capability,
    },
    lexer::{word::Word, Token},
    CompileError, Value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct FileInto {
    pub copy: bool,
    pub create: bool,
    pub folder: Value,
    pub flags: Vec<Value>,
    pub mailbox_id: Option<Value>,
    pub special_use: Option<Value>,
}

impl CompilerState<'_> {
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
                    self.validate_argument(
                        1,
                        Capability::Copy.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    copy = true;
                }
                Token::Tag(Word::Create) => {
                    self.validate_argument(
                        2,
                        Capability::Mailbox.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    create = true;
                }
                Token::Tag(Word::Flags) => {
                    self.validate_argument(
                        3,
                        Capability::Imap4Flags.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    flags = self.parse_strings(false)?;
                }
                Token::Tag(Word::MailboxId) => {
                    self.validate_argument(
                        4,
                        Capability::Mailbox.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mailbox_id = self.parse_string()?.into();
                }
                Token::Tag(Word::SpecialUse) => {
                    self.validate_argument(
                        5,
                        Capability::SpecialUse.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
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
