/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{word::Word, Token},
    CompileError, Value,
};

/*

include [LOCATION] [":once"] [":optional"] <value: string>
  LOCATION = ":personal" / ":global"

*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Include {
    pub location: Location,
    pub once: bool,
    pub optional: bool,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Location {
    Personal,
    Global,
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

        self.instructions.push(Instruction::Include(Include {
            location,
            once,
            optional,
            value,
        }));
        Ok(())
    }
}
