/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    CompileError, Value,
    grammar::instruction::{CompilerState, Instruction},
    lexer::{Token, word::Word},
};

/*

include [LOCATION] [":once"] [":optional"] <value: string>
  LOCATION = ":personal" / ":global"

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
pub(crate) struct Include {
    pub location: Location,
    pub once: bool,
    pub optional: bool,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub(crate) enum Location {
    Personal = 0,
    Global = 1,
}

impl CompilerState<'_> {
    pub(crate) fn parse_include(&mut self) -> Result<(), CompileError> {
        let value;
        let mut once = false;
        let mut optional = false;
        let mut location = Location::Personal;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Once) => {
                    self.validate_argument(1, None, token_info.line_num, token_info.line_pos)?;
                    once = true;
                }
                Token::Tag(Word::Optional) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    optional = true;
                }
                Token::Tag(Word::Personal) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    location = Location::Personal;
                }
                Token::Tag(Word::Global) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    location = Location::Global;
                }
                _ => {
                    value = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.instructions
            .push(Instruction::Include(Box::new(Include {
                location,
                once,
                optional,
                value,
            })));
        Ok(())
    }
}
