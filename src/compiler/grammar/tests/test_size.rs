/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::compiler::{
    grammar::instruction::CompilerState,
    lexer::{word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestSize {
    pub over: bool,
    pub limit: usize,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_size(&mut self) -> Result<Test, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        let over = match token_info.token {
            Token::Tag(Word::Over) => true,
            Token::Tag(Word::Under) => false,
            _ => {
                return Err(token_info.expected("':over' or ':under'"));
            }
        };
        let token_info = self.tokens.unwrap_next()?;
        if let Token::Number(limit) = token_info.token {
            Ok(Test::Size(TestSize {
                over,
                limit,
                is_not: false,
            }))
        } else {
            Err(token_info.expected("number"))
        }
    }
}
