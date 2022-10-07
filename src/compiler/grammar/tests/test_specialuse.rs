use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::CompilerState,
    lexer::{string::StringItem, Token},
    CompileError,
};

use crate::compiler::grammar::test::Test;

/*
   Usage:  specialuse_exists [<mailbox: string>]
                             <special-use-attrs: string-list>
*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestSpecialUseExists {
    pub mailbox: Option<StringItem>,
    pub attributes: Vec<StringItem>,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_specialuseexists(&mut self) -> Result<Test, CompileError> {
        let mut maybe_attributes = self.parse_strings()?;

        match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen)) => {
                if maybe_attributes.len() == 1 {
                    Ok(Test::SpecialUseExists(TestSpecialUseExists {
                        mailbox: maybe_attributes.pop(),
                        attributes: self.parse_strings()?,
                    }))
                } else {
                    Err(self
                        .tokens
                        .unwrap_next()?
                        .invalid("mailbox cannot be a list"))
                }
            }
            _ => Ok(Test::SpecialUseExists(TestSpecialUseExists {
                mailbox: None,
                attributes: maybe_attributes,
            })),
        }
    }
}
