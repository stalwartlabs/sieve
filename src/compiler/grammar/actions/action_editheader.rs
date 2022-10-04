use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{
        command::{Command, CompilerState},
        Comparator,
    },
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::MatchType;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DeleteHeader {
    pub index: Option<u16>,
    pub index_last: bool,
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub field_name: StringItem,
    pub value_patterns: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_addheader(&mut self) -> Result<(), CompileError> {
        let mut field_name = None;
        let value;
        let mut last = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Last) => {
                    last = true;
                }
                _ => {
                    let string = self.parse_string_token(token_info)?;
                    if field_name.is_none() {
                        field_name = string.into();
                    } else {
                        value = string;
                        break;
                    }
                }
            }
        }
        self.commands.push(Command::AddHeader(AddHeader {
            last,
            field_name: field_name.unwrap(),
            value,
        }));
        Ok(())
    }

    pub(crate) fn parse_deleteheader(&mut self) -> Result<(), CompileError> {
        let field_name: StringItem;
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut index = None;
        let mut index_last = false;

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
                _ => {
                    field_name = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        let cmd = Command::DeleteHeader(DeleteHeader {
            index,
            index_last,
            comparator,
            match_type,
            field_name,
            value_patterns: if let Some(Ok(
                Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
            )) = self.tokens.peek().map(|r| r.map(|t| &t.token))
            {
                self.parse_strings(match_type == MatchType::Matches)?
            } else {
                Vec::new()
            },
        });
        self.commands.push(cmd);
        Ok(())
    }
}
