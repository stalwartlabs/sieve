use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use super::action_vacation::Fcc;

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
    pub fcc: Option<Fcc>,
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
                    from = self.parse_string()?.into();
                }
                Token::Tag(Word::Message) => {
                    message = self.parse_string()?.into();
                }
                Token::Tag(Word::Importance) => {
                    importance = self.parse_string()?.into();
                }
                Token::Tag(Word::Options) => {
                    options = self.parse_strings(false)?;
                }
                Token::Tag(Word::Create) => {
                    create = true;
                }
                Token::Tag(Word::SpecialUse) => {
                    special_use = self.parse_string()?.into();
                }
                Token::Tag(Word::MailboxId) => {
                    mailbox_id = self.parse_string()?.into();
                }
                Token::Tag(Word::Fcc) => {
                    fcc = self.parse_string()?.into();
                }
                Token::Tag(Word::Flags) => {
                    flags = self.parse_strings(false)?;
                }
                _ => {
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

        self.commands.push(Command::Notify(Notify {
            method,
            from,
            importance,
            options,
            message,
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
        }));
        Ok(())
    }
}
