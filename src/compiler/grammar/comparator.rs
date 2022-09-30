use phf::phf_map;
use serde::{Deserialize, Serialize};

use crate::compiler::{lexer::tokenizer::Tokenizer, CompileError};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Comparator {
    Elbonia,
    Octet,
    AsciiCaseMap,
    AsciiNumeric,
    Other(String),
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_comparator(&mut self) -> Result<Comparator, CompileError> {
        let comparator = String::from_utf8(self.unwrap_static_string()?)
            .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
        Ok(if let Some(comparator) = COMPARATOR.get(&comparator) {
            comparator.clone()
        } else {
            Comparator::Other(comparator)
        })
    }
}

static COMPARATOR: phf::Map<&'static str, Comparator> = phf_map! {
    "i;octet" => Comparator::Octet,
    "i;ascii-casemap" => Comparator::AsciiCaseMap,
    "i;ascii-numeric" => Comparator::AsciiNumeric,
};
