use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

use super::{comparator::Comparator, MatchType};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AddHeader {
    pub last: bool,
    pub field_name: StringItem,
    pub value: StringItem,
}

/*
      Usage: "deleteheader" [":index" <fieldno: number> [":last"]]
                   [COMPARATOR] [MATCH-TYPE]
                   <field-name: string>
                   [<value-patterns: string-list>]

*/
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DeleteHeader {
    pub index: Option<u16>,
    pub index_last: bool,
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub field_name: StringItem,
    pub value_patterns: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_addheader(&mut self) -> Result<AddHeader, CompileError> {
        let mut field_name = None;
        let mut value = None;
        let mut last = false;

        while value.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Last) => {
                    last = true;
                }
                Token::String(string) => {
                    if field_name.is_none() {
                        field_name = string.into();
                    } else {
                        value = string.into();
                    }
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(AddHeader {
            last,
            field_name: field_name.unwrap(),
            value: value.unwrap(),
        })
    }

    pub(crate) fn parse_deleteheader(&mut self) -> Result<DeleteHeader, CompileError> {
        let mut field_name = None;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut index = None;
        let mut index_last = false;

        while field_name.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::Index) => {
                    index = (self.unwrap_number(u16::MAX as usize)? as u16).into();
                }
                Token::Tag(Word::Last) => {
                    index_last = true;
                }
                Token::String(string) => {
                    field_name = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(DeleteHeader {
            index,
            index_last,
            comparator,
            match_type,
            field_name: field_name.unwrap(),
            value_patterns: if let Some(Ok(Token::String(_) | Token::BracketOpen)) =
                self.peek().map(|r| r.map(|t| &t.token))
            {
                self.parse_strings(match_type == MatchType::Matches)?
            } else {
                Vec::new()
            },
        })
    }
}
