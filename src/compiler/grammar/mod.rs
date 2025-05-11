/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::fmt::Display;

use self::{expr::Expression, instruction::CompilerState};

use super::{
    lexer::{tokenizer::TokenInfo, word::Word, Token},
    CompileError, ErrorType, Regex, Value,
};

pub mod actions;
pub mod expr;
pub mod instruction;
pub mod test;
pub mod tests;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
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

    // Extensions
    Expressions,
    While,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum AddressPart {
    LocalPart,
    Domain,
    All,
    User,
    Detail,
    Name,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum MatchType {
    Is,
    Contains,
    Matches(u64),
    Regex(u64),
    Value(RelationalMatch),
    Count(RelationalMatch),
    List,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum RelationalMatch {
    Gt,
    Ge,
    Lt,
    Le,
    Eq,
    Ne,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum Comparator {
    Elbonia,
    Octet,
    AsciiCaseMap,
    AsciiNumeric,
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub struct Clear {
    pub(crate) local_vars_idx: u32,
    pub(crate) local_vars_num: u32,
    pub(crate) match_vars: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub struct Invalid {
    pub(crate) name: String,
    pub(crate) line_num: usize,
    pub(crate) line_pos: usize,
}

#[derive(Debug, Eq, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct While {
    pub expr: Vec<Expression>,
    pub jz_pos: usize,
}

impl CompilerState<'_> {
    #[inline(always)]
    pub fn expect_instruction_end(&mut self) -> Result<(), CompileError> {
        self.tokens.expect_token(Token::Semicolon)
    }

    pub fn ignore_instruction(&mut self) -> Result<(), CompileError> {
        // Skip entire instruction
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
                        return Err(token_info.expected("instruction"));
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
                self.block.match_test_pos.push(self.instructions.len());
                Ok(MatchType::Matches(0))
            }
            Word::Regex => {
                self.block.match_test_pos.push(self.instructions.len());
                Ok(MatchType::Regex(0))
            }
            Word::List => Ok(MatchType::List),
            _ => {
                let token_info = self.tokens.unwrap_next()?;
                if let Token::StringConstant(text) = &token_info.token {
                    if let Some(relational) = lookup_relational(text.to_string().as_ref()) {
                        return Ok(if word == Word::Value {
                            MatchType::Value(relational)
                        } else {
                            MatchType::Count(relational)
                        });
                    }
                }
                Err(token_info.expected("relational match"))
            }
        }
    }

    pub(crate) fn parse_comparator(&mut self) -> Result<Comparator, CompileError> {
        let comparator = self.tokens.expect_static_string()?;
        Ok(if let Some(comparator) = lookup_comparator(&comparator) {
            comparator
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
                            strings.push(string.into_string());
                        }
                        Token::Comma => (),
                        Token::BracketClose if !strings.is_empty() => break,
                        _ => return Err(token_info.expected("constant string")),
                    }
                }
                Ok(strings)
            }
            Token::StringConstant(string) => Ok(vec![string.into_string()]),
            _ => Err(token_info.expected("'[' or constant string")),
        }
    }

    pub fn parse_string(&mut self) -> Result<Value, CompileError> {
        let next_token = self.tokens.unwrap_next()?;
        match next_token.token {
            Token::StringConstant(s) => Ok(Value::from(s)),
            Token::StringVariable(s) => {
                self.tokenize_string(&s, true)
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

    pub(crate) fn parse_strings(&mut self, allow_empty: bool) -> Result<Vec<Value>, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => self.parse_string_list(allow_empty),
            Token::StringConstant(s) => Ok(vec![Value::from(s)]),
            Token::StringVariable(s) => {
                self.tokenize_string(&s, true)
                    .map(|s| vec![s])
                    .map_err(|error_type| CompileError {
                        line_num: token_info.line_num,
                        line_pos: token_info.line_pos,
                        error_type,
                    })
            }
            _ => Err(token_info.expected("'[' or string")),
        }
    }

    pub(crate) fn parse_string_token(
        &mut self,
        token_info: TokenInfo,
    ) -> Result<Value, CompileError> {
        match token_info.token {
            Token::StringConstant(s) => Ok(Value::from(s)),
            Token::StringVariable(s) => {
                self.tokenize_string(&s, true)
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
    ) -> Result<Vec<Value>, CompileError> {
        match token_info.token {
            Token::StringConstant(s) => Ok(vec![Value::from(s)]),
            Token::StringVariable(s) => {
                self.tokenize_string(&s, true)
                    .map(|s| vec![s])
                    .map_err(|error_type| CompileError {
                        line_num: token_info.line_num,
                        line_pos: token_info.line_pos,
                        error_type,
                    })
            }
            Token::BracketOpen => self.parse_string_list(false),
            _ => Err(token_info.expected("string")),
        }
    }

    pub(crate) fn parse_string_list(
        &mut self,
        allow_empty: bool,
    ) -> Result<Vec<Value>, CompileError> {
        let mut strings = Vec::new();
        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::StringConstant(s) => {
                    strings.push(Value::from(s));
                }
                Token::StringVariable(s) => {
                    strings.push(self.tokenize_string(&s, true).map_err(|error_type| {
                        CompileError {
                            line_num: token_info.line_num,
                            line_pos: token_info.line_pos,
                            error_type,
                        }
                    })?);
                }
                Token::Comma => (),
                Token::BracketClose if !strings.is_empty() || allow_empty => break,
                _ => return Err(token_info.expected("string or string list")),
            }
        }
        Ok(strings)
    }

    #[inline(always)]
    pub(crate) fn has_capability(&self, capability: &Capability) -> bool {
        [&self.block]
            .into_iter()
            .chain(self.block_stack.iter())
            .any(|b| b.capabilities.contains(capability))
            || (capability != &Capability::Ihave && self.compiler.no_capability_check)
    }

    #[inline(always)]
    pub(crate) fn reset_param_check(&mut self) {
        self.param_check.fill(false);
    }

    #[inline(always)]
    pub(crate) fn validate_argument(
        &mut self,
        arg_num: usize,
        capability: Option<Capability>,
        line_num: usize,
        line_pos: usize,
    ) -> Result<(), CompileError> {
        if arg_num > 0 {
            if let Some(param) = self.param_check.get_mut(arg_num - 1) {
                if !*param {
                    *param = true;
                } else {
                    return Err(CompileError {
                        line_num,
                        line_pos,
                        error_type: ErrorType::DuplicatedParameter,
                    });
                }
            } else {
                #[cfg(test)]
                panic!("Argument out of range {arg_num}");
            }
        }
        if let Some(capability) = capability {
            if !self.has_capability(&capability) {
                return Err(CompileError {
                    line_num,
                    line_pos,
                    error_type: ErrorType::UndeclaredCapability(capability),
                });
            }
        }

        Ok(())
    }

    pub(crate) fn validate_match(
        &mut self,
        match_type: &MatchType,
        key_list: &mut [Value],
    ) -> Result<(), CompileError> {
        if matches!(match_type, MatchType::Regex(_)) {
            for key in key_list {
                if let Value::Text(expr) = key {
                    match fancy_regex::Regex::new(expr) {
                        Ok(regex) => {
                            *key = Value::Regex(Regex::new(expr.to_string(), regex));
                        }
                        Err(err) => {
                            return Err(self
                                .tokens
                                .unwrap_next()?
                                .custom(ErrorType::InvalidRegex(format!("{expr}: {err}"))));
                        }
                    }
                }
            }
        }
        Ok(())
    }
}

impl Capability {
    pub fn parse(capability: &str) -> Capability {
        if let Some(capability) = lookup_capabilities(capability) {
            capability
        } else if let Some(comparator) = capability.strip_prefix("comparator-") {
            Capability::Comparator(Comparator::Other(comparator.to_string()))
        } else {
            Capability::Other(capability.to_string())
        }
    }

    pub fn all() -> &'static [Capability] {
        &[
            Capability::Envelope,
            Capability::EnvelopeDsn,
            Capability::EnvelopeDeliverBy,
            Capability::FileInto,
            Capability::EncodedCharacter,
            Capability::Comparator(Comparator::Elbonia),
            Capability::Comparator(Comparator::AsciiCaseMap),
            Capability::Comparator(Comparator::AsciiNumeric),
            Capability::Comparator(Comparator::Octet),
            Capability::Body,
            Capability::Convert,
            Capability::Copy,
            Capability::Relational,
            Capability::Date,
            Capability::Index,
            Capability::Duplicate,
            Capability::Variables,
            Capability::EditHeader,
            Capability::ForEveryPart,
            Capability::Mime,
            Capability::Replace,
            Capability::Enclose,
            Capability::ExtractText,
            Capability::Enotify,
            Capability::RedirectDsn,
            Capability::RedirectDeliverBy,
            Capability::Environment,
            Capability::Reject,
            Capability::Ereject,
            Capability::ExtLists,
            Capability::SubAddress,
            Capability::Vacation,
            Capability::VacationSeconds,
            Capability::Fcc,
            Capability::Mailbox,
            Capability::MailboxId,
            Capability::MboxMetadata,
            Capability::ServerMetadata,
            Capability::SpecialUse,
            Capability::Imap4Flags,
            Capability::Ihave,
            Capability::ImapSieve,
            Capability::Include,
            Capability::Regex,
            Capability::SpamTest,
            Capability::SpamTestPlus,
            Capability::VirusTest,
        ]
    }
}

fn lookup_relational(input: &str) -> Option<RelationalMatch> {
    hashify::tiny_map!(
        input.as_bytes(),
        "gt" => RelationalMatch::Gt,
        "ge" => RelationalMatch::Ge,
        "lt" => RelationalMatch::Lt,
        "le" => RelationalMatch::Le,
        "eq" => RelationalMatch::Eq,
        "ne" => RelationalMatch::Ne,
    )
}

fn lookup_comparator(input: &str) -> Option<Comparator> {
    hashify::tiny_map!(
        input.as_bytes(),
        "i;octet" => Comparator::Octet,
        "i;ascii-casemap" => Comparator::AsciiCaseMap,
        "i;ascii-numeric" => Comparator::AsciiNumeric,
    )
}

impl Invalid {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn line_num(&self) -> usize {
        self.line_num
    }

    pub fn line_pos(&self) -> usize {
        self.line_pos
    }
}

impl From<&str> for Capability {
    fn from(value: &str) -> Self {
        Capability::parse(value)
    }
}

impl From<String> for Capability {
    fn from(value: String) -> Self {
        Capability::parse(&value)
    }
}

impl Display for Capability {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Capability::Envelope => f.write_str("envelope"),
            Capability::EnvelopeDsn => f.write_str("envelope-dsn"),
            Capability::EnvelopeDeliverBy => f.write_str("envelope-deliverby"),
            Capability::FileInto => f.write_str("fileinto"),
            Capability::EncodedCharacter => f.write_str("encoded-character"),
            Capability::Comparator(Comparator::Elbonia) => f.write_str("comparator-elbonia"),
            Capability::Comparator(Comparator::Octet) => f.write_str("comparator-i;octet"),
            Capability::Comparator(Comparator::AsciiCaseMap) => {
                f.write_str("comparator-i;ascii-casemap")
            }
            Capability::Comparator(Comparator::AsciiNumeric) => {
                f.write_str("comparator-i;ascii-numeric")
            }
            Capability::Comparator(Comparator::Other(comparator)) => f.write_str(comparator),
            Capability::Body => f.write_str("body"),
            Capability::Convert => f.write_str("convert"),
            Capability::Copy => f.write_str("copy"),
            Capability::Relational => f.write_str("relational"),
            Capability::Date => f.write_str("date"),
            Capability::Index => f.write_str("index"),
            Capability::Duplicate => f.write_str("duplicate"),
            Capability::Variables => f.write_str("variables"),
            Capability::EditHeader => f.write_str("editheader"),
            Capability::ForEveryPart => f.write_str("foreverypart"),
            Capability::Mime => f.write_str("mime"),
            Capability::Replace => f.write_str("replace"),
            Capability::Enclose => f.write_str("enclose"),
            Capability::ExtractText => f.write_str("extracttext"),
            Capability::Enotify => f.write_str("enotify"),
            Capability::RedirectDsn => f.write_str("redirect-dsn"),
            Capability::RedirectDeliverBy => f.write_str("redirect-deliverby"),
            Capability::Environment => f.write_str("environment"),
            Capability::Reject => f.write_str("reject"),
            Capability::Ereject => f.write_str("ereject"),
            Capability::ExtLists => f.write_str("extlists"),
            Capability::SubAddress => f.write_str("subaddress"),
            Capability::Vacation => f.write_str("vacation"),
            Capability::VacationSeconds => f.write_str("vacation-seconds"),
            Capability::Fcc => f.write_str("fcc"),
            Capability::Mailbox => f.write_str("mailbox"),
            Capability::MailboxId => f.write_str("mailboxid"),
            Capability::MboxMetadata => f.write_str("mboxmetadata"),
            Capability::ServerMetadata => f.write_str("servermetadata"),
            Capability::SpecialUse => f.write_str("special-use"),
            Capability::Imap4Flags => f.write_str("imap4flags"),
            Capability::Ihave => f.write_str("ihave"),
            Capability::ImapSieve => f.write_str("imapsieve"),
            Capability::Include => f.write_str("include"),
            Capability::Regex => f.write_str("regex"),
            Capability::SpamTest => f.write_str("spamtest"),
            Capability::SpamTestPlus => f.write_str("spamtestplus"),
            Capability::VirusTest => f.write_str("virustest"),
            Capability::While => f.write_str("vnd.stalwart.while"),
            Capability::Expressions => f.write_str("vnd.stalwart.expressions"),
            Capability::Other(capability) => f.write_str(capability),
        }
    }
}

fn lookup_capabilities(input: &str) -> Option<Capability> {
    hashify::tiny_map!(
        input.as_bytes(),
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

        // Extensions
        "vnd.stalwart.while" => Capability::While,
        "vnd.stalwart.expressions" => Capability::Expressions,
    )
}
