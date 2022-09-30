use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, Token},
        CompileError,
    },
    runtime::StringItem,
};

use super::test::Test;

/*
   Usage:  specialuse_exists [<mailbox: string>]
                             <special-use-attrs: string-list>
*/

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestSpecialUseExists {
    pub mailbox: Option<StringItem>,
    pub attributes: Vec<StringItem>,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test_specialuseexists(&mut self) -> Result<Test, CompileError> {
        let mut maybe_attributes = self.parse_strings(false)?;

        match self.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::String(_) | Token::BracketOpen)) => {
                if maybe_attributes.len() == 1 {
                    Ok(Test::SpecialUseExists(TestSpecialUseExists {
                        mailbox: maybe_attributes.pop(),
                        attributes: self.parse_strings(false)?,
                    }))
                } else {
                    Err(self.unwrap_next()?.invalid("mailbox cannot be a list"))
                }
            }
            _ => Ok(Test::SpecialUseExists(TestSpecialUseExists {
                mailbox: None,
                attributes: maybe_attributes,
            })),
        }
    }
}
