use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
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
    pub index: Option<i32>,
    pub comparator: Comparator,
    pub match_type: MatchType,
    pub field_name: StringItem,
    pub value_patterns: Vec<StringItem>,
    pub mime_anychild: bool,
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
        self.instructions.push(Instruction::AddHeader(AddHeader {
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
        let mut mime = false;
        let mut mime_anychild = false;

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
                    index = (self.tokens.expect_number(u16::MAX as usize)? as i32).into();
                }
                Token::Tag(Word::Last) => {
                    index_last = true;
                }
                Token::Tag(Word::Mime) => {
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    mime_anychild = true;
                }
                _ => {
                    field_name = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.invalid("missing ':mime' tag"));
        }

        let cmd = Instruction::DeleteHeader(DeleteHeader {
            index: if index_last { index.map(|i| -i) } else { index },
            comparator,
            match_type,
            field_name,
            value_patterns: if let Some(Ok(
                Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
            )) = self.tokens.peek().map(|r| r.map(|t| &t.token))
            {
                self.parse_strings()?
            } else {
                Vec::new()
            },
            mime_anychild,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
