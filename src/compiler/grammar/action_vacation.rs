use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Vacation {
    pub period: Period,
    pub subject: Option<StringItem>,
    pub from: Option<StringItem>,
    pub handle: Option<StringItem>,
    pub addresses: Vec<StringItem>,
    pub mime: bool,
    pub fcc: Option<Fcc>,
    pub reason: StringItem,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Period {
    Days(u64),
    Seconds(u64),
    Default,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Fcc {
    pub fcc: StringItem,
    pub create: bool,
    pub flags: Vec<StringItem>,
    pub special_use: Option<StringItem>,
    pub mailbox_id: Option<StringItem>,
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

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_vacation(&mut self) -> Result<Vacation, CompileError> {
        let mut period = Period::Default;
        let mut subject = None;
        let mut from = None;
        let mut handle = None;
        let mut addresses = Vec::new();
        let mut mime = false;
        let mut reason = None;

        let mut fcc = None;
        let mut create = false;
        let mut flags = Vec::new();
        let mut special_use = None;
        let mut mailbox_id = None;

        while reason.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::Create) => {
                    create = true;
                }
                Token::Tag(Word::Days) => {
                    if period == Period::Default {
                        period = Period::Days(self.unwrap_number(u64::MAX as usize)? as u64);
                    } else {
                        return Err(
                            token_info.invalid("multiple ':days' or ':seconds' tags specified")
                        );
                    }
                }
                Token::Tag(Word::Seconds) => {
                    if period == Period::Default {
                        period = Period::Seconds(self.unwrap_number(u64::MAX as usize)? as u64);
                    } else {
                        return Err(
                            token_info.invalid("multiple ':days' or ':seconds' tags specified")
                        );
                    }
                }
                Token::Tag(Word::Subject) => {
                    subject = self.unwrap_string()?.into();
                }
                Token::Tag(Word::From) => {
                    from = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Handle) => {
                    handle = self.unwrap_string()?.into();
                }
                Token::Tag(Word::SpecialUse) => {
                    special_use = self.unwrap_string()?.into();
                }
                Token::Tag(Word::MailboxId) => {
                    mailbox_id = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Fcc) => {
                    fcc = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Flags) => {
                    flags = self.parse_strings(false)?;
                }
                Token::Tag(Word::Addresses) => {
                    addresses = self.parse_strings(false)?;
                }
                Token::String(string) => {
                    reason = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        if fcc.is_none()
            && (create || !flags.is_empty() || special_use.is_some() || mailbox_id.is_some())
        {
            return Err(self.unwrap_next()?.invalid("missing ':fcc' tag"));
        }

        Ok(Vacation {
            reason: reason.unwrap(),
            period,
            subject,
            from,
            handle,
            addresses,
            mime,
            fcc: if let Some(fcc) = fcc {
                Fcc {
                    fcc,
                    create,
                    flags,
                    special_use,
                    mailbox_id,
                }
                .into()
            } else {
                None
            },
        })
    }
}
