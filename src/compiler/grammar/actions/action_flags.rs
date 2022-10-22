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

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    lexer::{string::StringItem, word::Word, Token},
    CompileError, ErrorType,
};

use super::action_set::Variable;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct EditFlags {
    pub action: Action,
    pub name: Option<Variable>,
    pub flags: Vec<StringItem>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Action {
    Set,
    Add,
    Remove,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_flag_action(&mut self, word: Word) -> Result<(), CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        let action = match word {
            Word::SetFlag => Action::Set,
            Word::AddFlag => Action::Add,
            Word::RemoveFlag => Action::Remove,
            _ => unreachable!(),
        };

        let instruction = Instruction::EditFlags(
            match (
                &token_info.token,
                self.tokens.peek().map(|r| r.map(|t| &t.token)),
            ) {
                (
                    Token::StringConstant(_),
                    Some(Ok(
                        Token::StringConstant(_) | Token::StringVariable(_) | Token::BracketOpen,
                    )),
                ) => EditFlags {
                    name: self.parse_variable_name(token_info)?.into(),
                    flags: self.parse_strings()?,
                    action,
                },
                (Token::BracketOpen, _)
                | (
                    Token::StringConstant(_) | Token::StringVariable(_),
                    Some(Ok(Token::Semicolon)),
                ) => EditFlags {
                    name: None,
                    flags: self.parse_strings_token(token_info)?,
                    action,
                },
                _ => {
                    return Err(token_info.custom(ErrorType::InvalidArguments));
                }
            },
        );

        self.instructions.push(instruction);

        Ok(())
    }
}
