use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use super::action_set::Modifier;

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ForEveryPart {
    pub jz_pos: usize,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct Replace {
    pub subject: Option<StringItem>,
    pub from: Option<StringItem>,
    pub replacement: StringItem,
    pub mime: bool,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct Enclose {
    pub subject: Option<StringItem>,
    pub headers: Vec<StringItem>,
    pub value: StringItem,
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) struct ExtractText {
    pub modifiers: Vec<Modifier>,
    pub first: Option<usize>,
    pub varname: StringItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MimeOpts {
    Type,
    Subtype,
    ContentType,
    Param(Vec<StringItem>),
    None,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_replace(&mut self) -> Result<(), CompileError> {
        let mut subject = None;
        let mut from = None;
        let replacement;
        let mut mime = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::Subject) => {
                    subject = self.parse_string()?.into();
                }
                Token::Tag(Word::From) => {
                    from = self.parse_string()?.into();
                }
                _ => {
                    replacement = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.commands.push(Command::Replace(Replace {
            subject,
            from,
            replacement,
            mime,
        }));
        Ok(())
    }

    pub(crate) fn parse_enclose(&mut self) -> Result<(), CompileError> {
        let mut subject = None;
        let mut headers = Vec::new();
        let value;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Subject) => {
                    subject = self.parse_string()?.into();
                }
                Token::Tag(Word::Headers) => {
                    headers = self.parse_string_list()?;
                }
                _ => {
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.commands.push(Command::Enclose(Enclose {
            subject,
            headers,
            value,
        }));
        Ok(())
    }

    pub(crate) fn parse_extracttext(&mut self) -> Result<(), CompileError> {
        let mut modifiers = Vec::new();
        let mut first = None;
        let varname;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::First) => {
                    first = self.tokens.expect_number(usize::MAX)?.into();
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
                    let modifier = word.into();
                    if !modifiers.contains(&modifier) {
                        modifiers.push(modifier);
                    }
                }
                _ => {
                    varname = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        modifiers.sort_unstable_by(|a: &Modifier, b: &Modifier| b.cmp(a));

        self.commands.push(Command::ExtractText(ExtractText {
            modifiers,
            first,
            varname,
        }));
        Ok(())
    }

    pub(crate) fn parse_mimeopts(&mut self, opts: Word) -> Result<MimeOpts, CompileError> {
        Ok(match opts {
            Word::Type => MimeOpts::Type,
            Word::Subtype => MimeOpts::Subtype,
            Word::ContentType => MimeOpts::ContentType,
            Word::Param => MimeOpts::Param(self.parse_strings()?),
            _ => MimeOpts::None,
        })
    }
}
