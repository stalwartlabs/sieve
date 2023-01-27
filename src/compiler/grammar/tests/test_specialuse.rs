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
    pub is_not: bool,
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
