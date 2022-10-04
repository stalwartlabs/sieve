use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{actions::action_mime::MimeOpts, command::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestHeader {
    pub header_list: Vec<StringItem>,
    pub key_list: Vec<StringItem>,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub index: Option<u16>,
    pub index_last: bool,

    pub mime: bool,
    pub mime_opts: MimeOpts,
    pub mime_anychild: bool,

    pub list: bool,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_header(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_list = None;
        let key_list;
        let mut index = None;
        let mut index_last = false;

        let mut mime = false;
        let mut mime_opts = MimeOpts::None;
        let mut mime_anychild = false;

        let mut list = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
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
                    index = (self.tokens.expect_number(u16::MAX as usize)? as u16).into();
                }
                Token::Tag(Word::Last) => {
                    index_last = true;
                }
                Token::Tag(Word::List) => {
                    list = true;
                }
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    mime_anychild = true;
                }
                Token::Tag(
                    word @ (Word::Type | Word::Subtype | Word::ContentType | Word::Param),
                ) => {
                    mime_opts = self.parse_mimeopts(word)?;
                }
                _ => {
                    if header_list.is_none() {
                        header_list = self.parse_strings_token(token_info, false)?.into();
                    } else {
                        key_list =
                            self.parse_strings_token(token_info, match_type == MatchType::Matches)?;
                        break;
                    }
                }
            }
        }

        Ok(Test::Header(TestHeader {
            header_list: header_list.unwrap(),
            key_list,
            match_type,
            comparator,
            index,
            index_last,
            mime,
            mime_opts,
            mime_anychild,
            list,
        }))
    }
}
