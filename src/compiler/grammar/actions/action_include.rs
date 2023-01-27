/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
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
    pub value: StringItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Location {
    Personal,
    Global,
}

impl<'x> CompilerState<'x> {
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
