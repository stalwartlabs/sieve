use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use super::{action_set::Modifier, command::Command};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ForEveryPart {
    pub name: Option<Vec<u8>>,
    pub commands: Vec<Command>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Break {
    pub name: Option<Vec<u8>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Replace {
    pub subject: Option<StringItem>,
    pub from: Option<StringItem>,
    pub replacement: StringItem,
    pub mime: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Enclose {
    pub subject: Option<StringItem>,
    pub headers: Vec<StringItem>,
    pub value: StringItem,
}

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct ExtractText {
    pub modifiers: Vec<Modifier>,
    pub first: Option<usize>,
    pub varname: StringItem,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MimeOpts {
    Type,
    Subtype,
    ContentType,
    Param(Vec<StringItem>),
    None,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_replace(&mut self) -> Result<Replace, CompileError> {
        let mut subject = None;
        let mut from = None;
        let mut replacement = None;
        let mut mime = false;

        while replacement.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::Subject) => {
                    subject = self.unwrap_string()?.into();
                }
                Token::Tag(Word::From) => {
                    from = self.unwrap_string()?.into();
                }
                Token::String(string) => {
                    replacement = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(Replace {
            subject,
            from,
            replacement: replacement.unwrap(),
            mime,
        })
    }

    pub(crate) fn parse_enclose(&mut self) -> Result<Enclose, CompileError> {
        let mut subject = None;
        let mut headers = Vec::new();
        let mut value = None;

        while value.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Subject) => {
                    subject = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Headers) => {
                    headers = self.parse_string_list(false)?;
                }
                Token::String(string) => {
                    value = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(Enclose {
            subject,
            headers,
            value: value.unwrap(),
        })
    }

    pub(crate) fn parse_extracttext(&mut self) -> Result<ExtractText, CompileError> {
        let mut modifiers = Vec::new();
        let mut first = None;
        let mut varname = None;

        while varname.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::First) => {
                    first = self.unwrap_number(usize::MAX)?.into();
                }
                Token::Tag(
                    word @ (Word::Lower
                    | Word::Upper
                    | Word::LowerFirst
                    | Word::UpperFirst
                    | Word::QuoteWildcard
                    | Word::QuoteRegex
                    | Word::Length),
                ) => {
                    modifiers.push(word.into());
                }
                Token::String(string) => {
                    varname = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(ExtractText {
            modifiers,
            first,
            varname: varname.unwrap(),
        })
    }

    pub(crate) fn parse_mimeopts(&mut self, opts: Word) -> Result<MimeOpts, CompileError> {
        Ok(match opts {
            Word::Type => MimeOpts::Type,
            Word::Subtype => MimeOpts::Subtype,
            Word::ContentType => MimeOpts::ContentType,
            Word::Param => MimeOpts::Param(self.parse_strings(false)?),
            _ => MimeOpts::None,
        })
    }
}
