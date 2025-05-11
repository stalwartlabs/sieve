/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::compiler::{grammar::instruction::CompilerState, lexer::Token, CompileError, Value};

use crate::compiler::grammar::test::Test;

/*
   Usage:  specialuse_exists [<mailbox: string>]
                             <special-use-attrs: string-list>
*/

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestSpecialUseExists {
    pub mailbox: Option<Value>,
    pub attributes: Vec<Value>,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_specialuseexists(&mut self) -> Result<Test, CompileError> {
        let mut maybe_attributes = self.parse_strings(false)?;

        match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
            Some(Ok(Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen)) => {
                if maybe_attributes.len() == 1 {
                    Ok(Test::SpecialUseExists(TestSpecialUseExists {
                        mailbox: maybe_attributes.pop(),
                        attributes: self.parse_strings(false)?,
                        is_not: false,
                    }))
                } else {
                    Err(self.tokens.unwrap_next()?.expected("string"))
                }
            }
            _ => Ok(Test::SpecialUseExists(TestSpecialUseExists {
                mailbox: None,
                attributes: maybe_attributes,
                is_not: false,
            })),
        }
    }
}
