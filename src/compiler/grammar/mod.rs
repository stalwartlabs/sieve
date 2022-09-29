use phf::phf_map;
use serde::{Deserialize, Serialize};

use crate::runtime::StringItem;

use super::{
    lexer::{tokenizer::Tokenizer, word::Word, Token},
    CompileError,
};

pub mod action_convert;
pub mod action_editheader;
pub mod action_fileinto;
pub mod action_mime;
pub mod action_notify;
pub mod action_redirect;
pub mod action_require;
pub mod action_set;
pub mod capability;
pub mod command;
pub mod comparator;
pub mod string_list;
pub mod test;
pub mod test_address;
pub mod test_body;
pub mod test_date;
pub mod test_duplicate;
pub mod test_envelope;
pub mod test_exists;
pub mod test_header;
pub mod test_notify;
pub mod test_size;
pub mod test_string;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AddressPart {
    LocalPart,
    Domain,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MatchType {
    Is,
    Contains,
    Matches,
    Value(RelationalMatch),
    Count(RelationalMatch),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum RelationalMatch {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

impl<'x> Tokenizer<'x> {
    #[inline(always)]
    pub fn expect_command_end(&mut self) -> Result<(), CompileError> {
        self.expect_token(Token::Semicolon)
    }

    pub fn ignore_command(&mut self) -> Result<(), CompileError> {
        // Skip entire command
        let mut curly_count = 0;
        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Semicolon if curly_count == 0 => {
                    break;
                }
                Token::CurlyOpen => {
                    curly_count += 1;
                }
                Token::CurlyClose => match curly_count {
                    0 => {
                        return Err(token_info.expected("command"));
                    }
                    1 => {
                        break;
                    }
                    _ => curly_count -= 1,
                },
                _ => (),
            }
        }

        Ok(())
    }

    pub fn parse_match_type(&mut self, word: Word) -> Result<MatchType, CompileError> {
        match word {
            Word::Is => Ok(MatchType::Is),
            Word::Contains => Ok(MatchType::Contains),
            Word::Matches => Ok(MatchType::Matches),
            _ => {
                let token_info = self.unwrap_next()?;
                if let Token::String(StringItem::Text(text)) = &token_info.token {
                    if let Ok(text) = std::str::from_utf8(text) {
                        if let Some(relational) = RELATIONAL.get(text) {
                            return Ok(if word == Word::Value {
                                MatchType::Value(*relational)
                            } else {
                                MatchType::Count(*relational)
                            });
                        }
                    }
                }
                Err(token_info.expected("relational match"))
            }
        }
    }
}

static RELATIONAL: phf::Map<&'static str, RelationalMatch> = phf_map! {
    "gt" => RelationalMatch::Gt,
    "ge" => RelationalMatch::Ge,
    "lt" => RelationalMatch::Lt,
    "le" => RelationalMatch::Le,
    "eq" => RelationalMatch::Eq,
    "ne" => RelationalMatch::Ne,
};
