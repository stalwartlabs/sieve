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

use crate::compiler::grammar::instruction::{CompilerState, Instruction};
use crate::compiler::lexer::string::StringItem;
use crate::compiler::lexer::Token;
use crate::compiler::CompileError;
use crate::CommandType;

use crate::compiler::grammar::test::Test;
use crate::compiler::lexer::word::Word;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Execute {
    pub command: StringItem,
    pub command_type: CommandType,
    pub arguments: Vec<StringItem>,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Error {
    pub message: StringItem,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_execute(&mut self) -> Result<(), CompileError> {
        let instruction = Instruction::Execute(self.parse_execute_()?);
        self.instructions.push(instruction);
        Ok(())
    }

    pub(crate) fn parse_test_execute(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Execute(self.parse_execute_()?))
    }

    fn parse_execute_(&mut self) -> Result<Execute, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        Ok(Execute {
            command_type: match token_info.token {
                Token::Tag(Word::Binary) => CommandType::Binary,
                Token::Tag(Word::Query) => CommandType::Query,
                _ => {
                    return Err(token_info.expected("':binary' or ':query'"));
                }
            },
            command: self.parse_string()?,
            arguments: self.parse_strings()?,
            is_not: false,
        })
    }
}
