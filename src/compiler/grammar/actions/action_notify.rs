/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        grammar::{
            instruction::{CompilerState, Instruction},
            Capability,
        },
        lexer::{string::StringItem, word::Word, Token},
        CompileError,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Notify {
    pub from: Option<StringItem>,
    pub importance: Option<StringItem>,
    pub options: Vec<StringItem>,
    pub message: Option<StringItem>,
    pub fcc: Option<FileCarbonCopy<StringItem>>,
    pub method: StringItem,
}

impl<'x> CompilerState<'x> {
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
                    if let StringItem::Text(address) = &address {
                        if address.is_empty() || !validate_from(address) {
                            return Err(token_info.invalid("from address"));
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
                    options = self.parse_strings()?;
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
                    flags = self.parse_strings()?;
                }
                _ => {
                    if let Token::StringConstant(uri) = &token_info.token {
                        if validate_uri(std::str::from_utf8(uri).unwrap_or("")).is_none() {
                            return Err(token_info.invalid("URI"));
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
            return Err(self.tokens.unwrap_next()?.invalid("missing ':fcc' tag"));
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
