use phf::phf_map;
use serde::{Deserialize, Serialize};

use crate::runtime::eval::IntoString;

use self::command::CompilerState;

use super::{
    lexer::{string::StringItem, tokenizer::TokenInfo, word::Word, Token},
    CompileError,
};

pub mod actions;
pub mod command;
pub mod test;
pub mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Capability {
    Envelope,
    EnvelopeDsn,
    EnvelopeDeliverBy,
    FileInto,
    EncodedCharacter,
    Comparator(Comparator),
    Other(String),
    Body,
    Convert,
    Copy,
    Relational,
    Date,
    Index,
    Duplicate,
    Variables,
    EditHeader,
    ForEveryPart,
    Mime,
    Replace,
    Enclose,
    ExtractText,
    Enotify,
    RedirectDsn,
    RedirectDeliverBy,
    Environment,
    Reject,
    Ereject,
    ExtLists,
    SubAddress,
    Vacation,
    VacationSeconds,
    Fcc,
    Mailbox,
    MailboxId,
    MboxMetadata,
    ServerMetadata,
    SpecialUse,
    Imap4Flags,
    Ihave,
    ImapSieve,
    Include,
    Regex,
    SpamTest,
    SpamTestPlus,
    VirusTest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum AddressPart {
    LocalPart,
    Domain,
    All,
    User,
    Detail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum MatchType {
    Is,
    Contains,
    Matches(usize),
    Regex(usize),
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Comparator {
    Elbonia,
    Octet,
    AsciiCaseMap,
    AsciiNumeric,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Clear {
    pub(crate) local_vars_idx: u32,
    pub(crate) local_vars_num: u32,
    pub(crate) match_vars: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Invalid {
    pub(crate) name: String,
    pub(crate) line_num: usize,
    pub(crate) line_pos: usize,
}

impl<'x> CompilerState<'x> {
    #[inline(always)]
    pub fn expect_command_end(&mut self) -> Result<(), CompileError> {
        self.tokens.expect_token(Token::Semicolon)
    }

    pub fn ignore_command(&mut self) -> Result<(), CompileError> {
        // Skip entire command
        let mut curly_count = 0;
        loop {
            let token_info = self.tokens.unwrap_next()?;
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

    pub fn ignore_test(&mut self) -> Result<(), CompileError> {
        let mut d_count = 0;
        while let Some(token_info) = self.tokens.peek() {
            match token_info?.token {
                Token::ParenthesisOpen => {
                    d_count += 1;
                }
                Token::ParenthesisClose => {
                    if d_count == 0 {
                        break;
                    } else {
                        d_count -= 1;
                    }
                }
                Token::Comma => {
                    if d_count == 0 {
                        break;
                    }
                }
                Token::CurlyOpen => {
                    break;
                }
                _ => (),
            }
            self.tokens.next();
        }

        Ok(())
    }

    pub fn parse_match_type(&mut self, word: Word) -> Result<MatchType, CompileError> {
        match word {
            Word::Is => Ok(MatchType::Is),
            Word::Contains => Ok(MatchType::Contains),
            Word::Matches => {
                self.match_test_pos_last = self.commands.len();
                Ok(MatchType::Matches(0))
            }
            Word::Regex => {
                self.match_test_pos_last = self.commands.len();
                Ok(MatchType::Regex(0))
            }
            _ => {
                let token_info = self.tokens.unwrap_next()?;
                if let Token::StringConstant(text) = &token_info.token {
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

    pub(crate) fn parse_comparator(&mut self) -> Result<Comparator, CompileError> {
        let comparator = self.tokens.expect_static_string()?.into_string();
        Ok(if let Some(comparator) = COMPARATOR.get(&comparator) {
            comparator.clone()
        } else {
            Comparator::Other(comparator)
        })
    }

    pub(crate) fn parse_static_strings(&mut self) -> Result<Vec<String>, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => {
                let mut strings = Vec::new();
                loop {
                    let token_info = self.tokens.unwrap_next()?;
                    match token_info.token {
                        Token::StringConstant(string) => {
                            strings.push(String::from_utf8(string).map_err(|err| {
                                TokenInfo {
                                    token: Token::StringConstant(err.into_bytes()),
                                    line_num: token_info.line_num,
                                    line_pos: token_info.line_pos,
                                }
                                .invalid_utf8()
                            })?);
                        }
                        Token::Comma => (),
                        Token::BracketClose if !strings.is_empty() => break,
                        _ => return Err(token_info.expected("constant string")),
                    }
                }
                Ok(strings)
            }
            Token::StringConstant(string) => {
                Ok(vec![String::from_utf8(string).map_err(|err| {
                    TokenInfo {
                        token: Token::StringConstant(err.into_bytes()),
                        line_num: token_info.line_num,
                        line_pos: token_info.line_pos,
                    }
                    .invalid_utf8()
                })?])
            }
            _ => Err(token_info.expected("'[' or constant string")),
        }
    }

    pub fn parse_string(&mut self) -> Result<StringItem, CompileError> {
        let next_token = self.tokens.unwrap_next()?;
        match next_token.token {
            Token::StringConstant(s) => Ok(StringItem::Text(s.into_string())),
            Token::StringVariable(s) => {
                self.tokenize_string(&s, true, false)
                    .map_err(|error_type| CompileError {
                        line_num: next_token.line_num,
                        line_pos: next_token.line_pos,
                        error_type,
                    })
            }
            Token::BracketOpen => {
                let mut items = self.parse_string_list(false)?;
                match items.pop() {
                    Some(s) if items.is_empty() => Ok(s),
                    _ => Err(next_token.expected("string")),
                }
            }
            _ => Err(next_token.expected("string")),
        }
    }

    pub(crate) fn parse_strings(
        &mut self,
        parse_matches: bool,
    ) -> Result<Vec<StringItem>, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => self.parse_string_list(parse_matches),
            Token::StringConstant(s) if !parse_matches => {
                Ok(vec![StringItem::Text(s.into_string())])
            }
            Token::StringConstant(s) | Token::StringVariable(s) => self
                .tokenize_string(&s, true, parse_matches)
                .map(|s| vec![s])
                .map_err(|error_type| CompileError {
                    line_num: token_info.line_num,
                    line_pos: token_info.line_pos,
                    error_type,
                }),
            _ => Err(token_info.expected("'[' or string")),
        }
    }

    pub(crate) fn parse_string_token(
        &mut self,
        token_info: TokenInfo,
    ) -> Result<StringItem, CompileError> {
        match token_info.token {
            Token::StringConstant(s) => Ok(StringItem::Text(s.into_string())),
            Token::StringVariable(s) => {
                self.tokenize_string(&s, true, false)
                    .map_err(|error_type| CompileError {
                        line_num: token_info.line_num,
                        line_pos: token_info.line_pos,
                        error_type,
                    })
            }
            _ => Err(token_info.expected("string")),
        }
    }

    pub(crate) fn parse_strings_token(
        &mut self,
        token_info: TokenInfo,
        parse_matches: bool,
    ) -> Result<Vec<StringItem>, CompileError> {
        match token_info.token {
            Token::StringConstant(s) if !parse_matches => {
                Ok(vec![StringItem::Text(s.into_string())])
            }
            Token::StringConstant(s) | Token::StringVariable(s) => self
                .tokenize_string(&s, true, parse_matches)
                .map(|s| vec![s])
                .map_err(|error_type| CompileError {
                    line_num: token_info.line_num,
                    line_pos: token_info.line_pos,
                    error_type,
                }),
            Token::BracketOpen => self.parse_string_list(parse_matches),
            _ => Err(token_info.expected("string")),
        }
    }

    pub(crate) fn parse_string_list(
        &mut self,
        parse_matches: bool,
    ) -> Result<Vec<StringItem>, CompileError> {
        let mut strings = Vec::new();
        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::StringConstant(s) if !parse_matches => {
                    strings.push(StringItem::Text(s.into_string()));
                }
                Token::StringConstant(s) | Token::StringVariable(s) => {
                    strings.push(self.tokenize_string(&s, true, parse_matches).map_err(
                        |error_type| CompileError {
                            line_num: token_info.line_num,
                            line_pos: token_info.line_pos,
                            error_type,
                        },
                    )?);
                }
                Token::Comma => (),
                Token::BracketClose if !strings.is_empty() => break,
                _ => return Err(token_info.expected("string or string list")),
            }
        }
        Ok(strings)
    }
}

impl MatchType {
    pub fn is_matches(&self) -> bool {
        matches!(self, MatchType::Matches(_))
    }
}

impl Capability {
    pub fn parse(bytes: Vec<u8>) -> Capability {
        if let Some(capability) = CAPABILITIES.get(std::str::from_utf8(&bytes).unwrap_or("")) {
            capability.clone()
        } else {
            let capability = bytes.into_string();
            if let Some(comparator) = capability.strip_prefix("comparator-") {
                Capability::Comparator(Comparator::Other(comparator.to_string()))
            } else {
                Capability::Other(capability)
            }
        }
    }
}

impl From<String> for Capability {
    fn from(capability: String) -> Self {
        if let Some(capability) = CAPABILITIES.get(&capability) {
            capability.clone()
        } else if let Some(comparator) = capability.strip_prefix("comparator-") {
            Capability::Comparator(Comparator::Other(comparator.to_string()))
        } else {
            Capability::Other(capability)
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

static COMPARATOR: phf::Map<&'static str, Comparator> = phf_map! {
    "i;octet" => Comparator::Octet,
    "i;ascii-casemap" => Comparator::AsciiCaseMap,
    "i;ascii-numeric" => Comparator::AsciiNumeric,
};

static CAPABILITIES: phf::Map<&'static str, Capability> = phf_map! {
    "envelope" => Capability::Envelope,
    "envelope-dsn" => Capability::EnvelopeDsn,
    "envelope-deliverby" => Capability::EnvelopeDeliverBy,
    "fileinto" => Capability::FileInto,
    "encoded-character" => Capability::EncodedCharacter,
    "comparator-elbonia" => Capability::Comparator(Comparator::Elbonia),
    "comparator-i;octet" => Capability::Comparator(Comparator::Octet),
    "comparator-i;ascii-casemap" => Capability::Comparator(Comparator::AsciiCaseMap),
    "comparator-i;ascii-numeric" => Capability::Comparator(Comparator::AsciiNumeric),
    "body" => Capability::Body,
    "convert" => Capability::Convert,
    "copy" => Capability::Copy,
    "relational" => Capability::Relational,
    "date" => Capability::Date,
    "index" => Capability::Index,
    "duplicate" => Capability::Duplicate,
    "variables" => Capability::Variables,
    "editheader" => Capability::EditHeader,
    "foreverypart" => Capability::ForEveryPart,
    "mime" => Capability::Mime,
    "replace" => Capability::Replace,
    "enclose" => Capability::Enclose,
    "extracttext" => Capability::ExtractText,
    "enotify" => Capability::Enotify,
    "redirect-dsn" => Capability::RedirectDsn,
    "redirect-deliverby" => Capability::RedirectDeliverBy,
    "environment" => Capability::Environment,
    "reject" => Capability::Reject,
    "ereject" => Capability::Ereject,
    "extlists" => Capability::ExtLists,
    "subaddress" => Capability::SubAddress,
    "vacation" => Capability::Vacation,
    "vacation-seconds" => Capability::VacationSeconds,
    "fcc" => Capability::Fcc,
    "mailbox" => Capability::Mailbox,
    "mailboxid" => Capability::MailboxId,
    "mboxmetadata" => Capability::MboxMetadata,
    "servermetadata" => Capability::ServerMetadata,
    "special-use" => Capability::SpecialUse,
    "imap4flags" => Capability::Imap4Flags,
    "ihave" => Capability::Ihave,
    "imapsieve" => Capability::ImapSieve,
    "include" => Capability::Include,
    "regex" => Capability::Regex,
    "spamtest" => Capability::SpamTest,
    "spamtestplus" => Capability::SpamTestPlus,
    "virustest" => Capability::VirusTest,
};
