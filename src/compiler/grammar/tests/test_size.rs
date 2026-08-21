/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    CompileError,
    grammar::instruction::CompilerState,
    lexer::{Token, word::Word},
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
    pub limit: u64,
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
                limit: limit as u64,
                is_not: false,
            }))
        } else {
            Err(token_info.expected("number"))
        }
    }
}
