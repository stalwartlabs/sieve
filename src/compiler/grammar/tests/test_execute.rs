/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
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
use crate::compiler::CompileError;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Execute {
    pub command: StringItem,
    pub arguments: Vec<StringItem>,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Error {
    pub message: StringItem,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_execute(&mut self) -> Result<(), CompileError> {
        let command = Execute {
            command: self.parse_string()?,
            arguments: self.parse_strings()?,
            is_not: false,
        };
        self.instructions.push(Instruction::Execute(command));

        Ok(())
    }

    pub(crate) fn parse_test_execute(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Execute(Execute {
            command: self.parse_string()?,
            arguments: self.parse_strings()?,
            is_not: false,
        }))
    }
}
