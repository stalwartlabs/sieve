use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{instruction::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, AddressPart, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestEnvelope {
    pub header_list: Vec<StringItem>,
    pub key_list: Vec<StringItem>,
    pub address_part: AddressPart,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub zone: Option<i32>,
    pub list: bool,
    pub is_not: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_envelope(&mut self) -> Result<Test, CompileError> {
        let mut address_part = AddressPart::All;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_list = None;
        let key_list;
        let mut zone = None;

        let mut list = false;

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
                    | Word::Regex),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::List) => {
                    list = true;
                }
                Token::Tag(Word::Zone) => {
                    let token_info = self.tokens.unwrap_next()?;
                    if let Token::StringConstant(value) = &token_info.token {
                        if let Ok(value) = std::str::from_utf8(value) {
                            if let Ok(timezone) = value.parse::<i32>() {
                                zone = timezone.into();
                                continue;
                            }
                        }
                    }
                    return Err(token_info.expected("string containing time zone"));
                }
                _ => {
                    if header_list.is_none() {
                        header_list = self.parse_strings_token(token_info)?.into();
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }

        Ok(Test::Envelope(TestEnvelope {
            header_list: header_list.unwrap(),
            key_list,
            address_part,
            match_type,
            comparator,
            zone,
            list,
            is_not: false,
        }))
    }
}
