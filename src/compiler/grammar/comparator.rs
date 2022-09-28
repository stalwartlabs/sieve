use phf::phf_map;

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, Token},
        CompileError,
    },
    runtime::StringItem,
    Comparator,
};

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_comparator(&mut self) -> Result<Comparator, CompileError> {
        let token_info = self.unwrap_next()?;
        if let Token::String(StringItem::Text(comparator)) = token_info.token {
            let comparator = String::from_utf8(comparator)
                .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
            Ok(if let Some(comparator) = COMPARATOR.get(&comparator) {
                comparator.clone()
            } else {
                Comparator::Other(comparator)
            })
        } else {
            Err(token_info.expected("string"))
        }
    }
}

static COMPARATOR: phf::Map<&'static str, Comparator> = phf_map! {
    "i;octet" => Comparator::Octet,
    "i;ascii-casemap" => Comparator::AsciiCaseMap,
    "i;ascii-numeric" => Comparator::AsciiNumeric,
};
