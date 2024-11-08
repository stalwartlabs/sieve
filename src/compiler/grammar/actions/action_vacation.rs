/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
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
            test::Test,
            Capability,
        },
        lexer::{word::Word, Token},
        CompileError, Value,
    },
    FileCarbonCopy,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vacation {
    pub subject: Option<Value>,
    pub from: Option<Value>,
    pub mime: bool,
    pub fcc: Option<FileCarbonCopy<Value>>,
    pub reason: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TestVacation {
    pub addresses: Vec<Value>,
    pub period: Period,
    pub handle: Option<Value>,
    pub reason: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Period {
    Days(u64),
    Seconds(u64),
    Default,
}

/*

vacation [":days" number] [":subject" string]
                     [":from" string] [":addresses" string-list]
                     [":mime"] [":handle" string] <reason: string>

vacation [FCC]
                     [":days" number | ":seconds" number]
                     [":subject" string]
                     [":from" string]
                     [":addresses" string-list]
                     [":mime"]
                     [":handle" string]
                     <reason: string>

":flags" <list-of-flags: string-list>


   FCC         = ":fcc" string *FCC-OPTS
                   ; per Section 2.6.2 of RFC 5228,
                   ; the tagged arguments in FCC may appear in any order

   FCC-OPTS    = CREATE / IMAP-FLAGS / SPECIAL-USE
                   ; each option MUST NOT appear more than once

   CREATE      = ":create"
   IMAP-FLAGS  = ":flags" string-list
   SPECIAL-USE = ":specialuse" string
*/

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_vacation(&mut self) -> Result<(), CompileError> {
        let mut period = Period::Default;
        let mut subject = None;
        let mut from = None;
        let mut handle = None;
        let mut addresses = Vec::new();
        let mut mime = false;
        let reason;

        let mut fcc = None;
        let mut create = false;
        let mut flags = Vec::new();
        let mut special_use = None;
        let mut mailbox_id = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    mime = true;
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
                Token::Tag(Word::Days) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    period = Period::Days(self.tokens.expect_number(u64::MAX as usize)? as u64);
                }
                Token::Tag(Word::Seconds) => {
                    self.validate_argument(
                        3,
                        Capability::VacationSeconds.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    period = Period::Seconds(self.tokens.expect_number(u64::MAX as usize)? as u64);
                }
                Token::Tag(Word::Subject) => {
                    self.validate_argument(4, None, token_info.line_num, token_info.line_pos)?;
                    subject = self.parse_string()?.into();
                }
                Token::Tag(Word::From) => {
                    self.validate_argument(5, None, token_info.line_num, token_info.line_pos)?;
                    from = self.parse_string()?.into();
                }
                Token::Tag(Word::Handle) => {
                    self.validate_argument(6, None, token_info.line_num, token_info.line_pos)?;
                    handle = self.parse_string()?.into();
                }
                Token::Tag(Word::SpecialUse) => {
                    self.validate_argument(
                        7,
                        Capability::SpecialUse.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    special_use = self.parse_string()?.into();
                }
                Token::Tag(Word::MailboxId) => {
                    self.validate_argument(
                        8,
                        Capability::MailboxId.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mailbox_id = self.parse_string()?.into();
                }
                Token::Tag(Word::Fcc) => {
                    self.validate_argument(
                        9,
                        Capability::Fcc.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    fcc = self.parse_string()?.into();
                }
                Token::Tag(Word::Flags) => {
                    self.validate_argument(
                        10,
                        Capability::Imap4Flags.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    flags = self.parse_strings(false)?;
                }
                Token::Tag(Word::Addresses) => {
                    self.validate_argument(11, None, token_info.line_num, token_info.line_pos)?;
                    addresses = self.parse_strings(false)?;
                }
                _ => {
                    reason = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        if fcc.is_none()
            && (create || !flags.is_empty() || special_use.is_some() || mailbox_id.is_some())
        {
            return Err(self.tokens.unwrap_next()?.missing_tag(":fcc"));
        }

        self.instructions
            .push(Instruction::Test(Test::Vacation(TestVacation {
                period,
                handle,
                reason: reason.clone(),
                addresses,
            })));

        self.instructions
            .push(Instruction::Jz(self.instructions.len() + 2));

        self.instructions.push(Instruction::Vacation(Vacation {
            reason,
            subject,
            from,
            mime,
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
