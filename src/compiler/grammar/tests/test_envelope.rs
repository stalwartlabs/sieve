use phf::phf_map;
use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        grammar::{instruction::CompilerState, Comparator},
        lexer::{string::StringItem, word::Word, Token},
        CompileError,
    },
    runtime::string::IntoString,
    Envelope,
};

use crate::compiler::grammar::{test::Test, AddressPart, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestEnvelope {
    pub envelope_list: Vec<Envelope>,
    pub key_list: Vec<StringItem>,
    pub address_part: AddressPart,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub zone: Option<i64>,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_envelope(&mut self) -> Result<Test, CompileError> {
        let mut address_part = AddressPart::All;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut envelope_list = None;
        let key_list;
        let mut zone = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::LocalPart | Word::Domain | Word::All | Word::User | Word::Detail),
                ) => {
                    address_part = word.into();
                }
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex
                    | Word::List),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::Zone) => {
                    zone = self.parse_timezone()?.into();
                }
                _ => {
                    if envelope_list.is_none() {
                        let mut envelopes = Vec::new();
                        match token_info.token {
                            Token::StringConstant(s) => {
                                envelopes.push(s.into_string().into());
                            }
                            Token::BracketOpen => loop {
                                let token_info = self.tokens.unwrap_next()?;
                                match token_info.token {
                                    Token::StringConstant(s) => {
                                        envelopes.push(s.into_string().into());
                                    }
                                    Token::Comma => (),
                                    Token::BracketClose if !envelopes.is_empty() => break,
                                    _ => return Err(token_info.expected("constant string")),
                                }
                            },
                            _ => return Err(token_info.expected("constant string")),
                        }

                        envelope_list = envelopes.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }

        Ok(Test::Envelope(TestEnvelope {
            envelope_list: envelope_list.unwrap(),
            key_list,
            address_part,
            match_type,
            comparator,
            zone,
            is_not: false,
        }))
    }
}

impl From<String> for Envelope {
    fn from(name: String) -> Self {
        if let Some(envelope) = ENVELOPE.get(&name) {
            envelope.clone()
        } else {
            Envelope::Other(name.to_lowercase())
        }
    }
}

impl From<&str> for Envelope {
    fn from(name: &str) -> Self {
        if let Some(envelope) = ENVELOPE.get(name) {
            envelope.clone()
        } else {
            Envelope::Other(name.to_lowercase())
        }
    }
}

pub(crate) static ENVELOPE: phf::Map<&'static str, Envelope> = phf_map! {
    "from" => Envelope::From,
    "to" => Envelope::To,
    "bytimeabsolute" => Envelope::ByTimeAbsolute,
    "bytimerelative" => Envelope::ByTimeRelative,
    "bymode" => Envelope::ByMode,
    "bytrace" => Envelope::ByTrace,
    "notify" => Envelope::Notify,
    "orcpt" => Envelope::Orcpt,
    "ret" => Envelope::Ret,
    "envid" => Envelope::Envid,
};
