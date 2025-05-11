/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::{
    compiler::{
        grammar::{
            instruction::{CompilerState, Instruction, MapLocalVars},
            Capability,
        },
        lexer::{word::Word, Token},
        CompileError, ErrorType, Value,
    },
    runtime::actions::action_notify::{validate_from, validate_uri},
    FileCarbonCopy,
};

/*
notify [":from" string]
           [":importance" <"1" / "2" / "3">]
           [":options" string-list]
           [":message" string]
           <method: string>

*/

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct Notify {
    pub from: Option<Value>,
    pub importance: Option<Value>,
    pub options: Vec<Value>,
    pub message: Option<Value>,
    pub fcc: Option<FileCarbonCopy<Value>>,
    pub method: Value,
}

impl CompilerState<'_> {
    pub(crate) fn parse_notify(&mut self) -> Result<(), CompileError> {
        let method;
        let mut from = None;
        let mut importance = None;
        let mut message = None;
        let mut options = Vec::new();

        let mut fcc = None;
        let mut create = false;
        let mut flags = Vec::new();
        let mut special_use = None;
        let mut mailbox_id = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::From) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    let address = self.parse_string()?;
                    if let Value::Text(address) = &address {
                        if address.is_empty() || !validate_from(address) {
                            return Err(token_info.custom(ErrorType::InvalidAddress));
                        }
                    }
                    from = address.into();
                }
                Token::Tag(Word::Message) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    message = self.parse_string()?.into();
                }
                Token::Tag(Word::Importance) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    importance = self.parse_string()?.into();
                }
                Token::Tag(Word::Options) => {
                    self.validate_argument(4, None, token_info.line_num, token_info.line_pos)?;
                    options = self.parse_strings(false)?;
                }
                Token::Tag(Word::Create) => {
                    self.validate_argument(
                        5,
                        Capability::Mailbox.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    create = true;
                }
                Token::Tag(Word::SpecialUse) => {
                    self.validate_argument(
                        6,
                        Capability::SpecialUse.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    special_use = self.parse_string()?.into();
                }
                Token::Tag(Word::MailboxId) => {
                    self.validate_argument(
                        7,
                        Capability::MailboxId.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mailbox_id = self.parse_string()?.into();
                }
                Token::Tag(Word::Fcc) => {
                    self.validate_argument(
                        8,
                        Capability::Fcc.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    fcc = self.parse_string()?.into();
                }
                Token::Tag(Word::Flags) => {
                    self.validate_argument(
                        9,
                        Capability::Imap4Flags.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    flags = self.parse_strings(false)?;
                }
                _ => {
                    if let Token::StringConstant(uri) = &token_info.token {
                        if validate_uri(uri.to_string().as_ref()).is_none() {
                            return Err(token_info.custom(ErrorType::InvalidURI));
                        }
                    }

                    method = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        if fcc.is_none()
            && (create || !flags.is_empty() || special_use.is_some() || mailbox_id.is_some())
        {
            return Err(self.tokens.unwrap_next()?.missing_tag(":fcc"));
        }

        self.instructions.push(Instruction::Notify(Notify {
            method,
            from,
            importance,
            options,
            message,
            fcc: if let Some(fcc) = fcc {
                FileCarbonCopy {
                    mailbox: fcc,
                    create,
                    flags,
                    special_use,
                    mailbox_id,
                }
                .into()
            } else {
                None
            },
        }));
        Ok(())
    }
}

impl MapLocalVars for FileCarbonCopy<Value> {
    fn map_local_vars(&mut self, last_id: usize) {
        self.mailbox.map_local_vars(last_id);
        self.mailbox_id.map_local_vars(last_id);
        self.flags.map_local_vars(last_id);
        self.special_use.map_local_vars(last_id);
    }
}
