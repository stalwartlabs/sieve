use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Modifier {
    Lower,
    Upper,
    LowerFirst,
    UpperFirst,
    QuoteWildcard,
    Length,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Set {
    pub modifiers: Vec<Modifier>,
    pub name: StringItem,
    pub value: StringItem,
    pub encode_url: bool,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_set(&mut self) -> Result<Set, CompileError> {
        let mut modifiers = Vec::new();
        let mut name = None;
        let mut value = None;
        let mut encode_url = false;

        while value.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Lower
                    | Word::Upper
                    | Word::LowerFirst
                    | Word::UpperFirst
                    | Word::QuoteWildcard
                    | Word::Length),
                ) => {
                    modifiers.push(word.into());
                }
                Token::Tag(Word::EncodeUrl) => {
                    encode_url = true;
                }
                Token::String(string) => {
                    if name.is_none() {
                        name = string.into();
                    } else {
                        value = string.into();
                    }
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(Set {
            modifiers,
            name: name.unwrap(),
            value: value.unwrap(),
            encode_url,
        })
    }
}

impl From<Word> for Modifier {
    fn from(word: Word) -> Self {
        match word {
            Word::Lower => Modifier::Lower,
            Word::Under => Modifier::Upper,
            Word::LowerFirst => Modifier::LowerFirst,
            Word::UpperFirst => Modifier::UpperFirst,
            Word::QuoteWildcard => Modifier::QuoteWildcard,
            Word::Length => Modifier::Length,
            _ => unreachable!(),
        }
    }
}
