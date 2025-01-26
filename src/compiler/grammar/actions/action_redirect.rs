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
    grammar::{
        instruction::{CompilerState, Instruction, MapLocalVars},
        Capability,
    },
    lexer::{word::Word, Token},
    CompileError, Value,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Redirect {
    pub copy: bool,
    pub address: Value,
    pub notify: Notify,
    pub return_of_content: Ret,
    pub by_time: ByTime<Value>,
    pub list: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum NotifyItem {
    Success,
    Failure,
    Delay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Notify {
    Never,
    Items(Vec<NotifyItem>),
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum Ret {
    Full,
    Hdrs,
    Default,
}

/*

   Usage:   redirect [:bytimerelative <rlimit: number> /
                      :bytimeabsolute <alimit:string>
                      [:bymode "notify"|"return"] [:bytrace]]
                     <address: string>

*/

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ByTime<T> {
    Relative {
        rlimit: u64,
        mode: ByMode,
        trace: bool,
    },
    Absolute {
        alimit: T,
        mode: ByMode,
        trace: bool,
    },
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub enum ByMode {
    Notify,
    Return,
    Default,
}

impl CompilerState<'_> {
    pub(crate) fn parse_redirect(&mut self) -> Result<(), CompileError> {
        let address;
        let mut copy = false;
        let mut ret = Ret::Default;
        let mut notify = Notify::Default;
        let mut list = false;
        let mut by_mode = ByMode::Default;
        let mut by_trace = false;
        let mut by_rlimit = None;
        let mut by_alimit = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Copy) => {
                    self.validate_argument(
                        1,
                        Capability::Copy.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    copy = true;
                }
                Token::Tag(Word::List) => {
                    self.validate_argument(
                        2,
                        Capability::ExtLists.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    list = true;
                }
                Token::Tag(Word::ByTrace) => {
                    self.validate_argument(
                        3,
                        Capability::RedirectDeliverBy.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    by_trace = true;
                }
                Token::Tag(Word::ByMode) => {
                    self.validate_argument(
                        4,
                        Capability::RedirectDeliverBy.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    let by_mode_ = self.tokens.expect_static_string()?;
                    if by_mode_.eq_ignore_ascii_case("notify") {
                        by_mode = ByMode::Notify;
                    } else if by_mode_.eq_ignore_ascii_case("return") {
                        by_mode = ByMode::Return;
                    } else {
                        return Err(token_info.expected("\"notify\" or \"return\""));
                    }
                }
                Token::Tag(Word::ByTimeRelative) => {
                    self.validate_argument(
                        5,
                        Capability::RedirectDeliverBy.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    by_rlimit = (self.tokens.expect_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::ByTimeAbsolute) => {
                    self.validate_argument(
                        5,
                        Capability::RedirectDeliverBy.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    by_alimit = self.parse_string()?.into();
                }
                Token::Tag(Word::Ret) => {
                    self.validate_argument(
                        6,
                        Capability::RedirectDsn.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    let ret_ = self.tokens.expect_static_string()?;
                    if ret_.eq_ignore_ascii_case("full") {
                        ret = Ret::Full;
                    } else if ret_.eq_ignore_ascii_case("hdrs") {
                        ret = Ret::Hdrs;
                    } else {
                        return Err(token_info.expected("\"FULL\" or \"HDRS\""));
                    }
                }
                Token::Tag(Word::Notify) => {
                    self.validate_argument(
                        7,
                        Capability::RedirectDsn.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    let notify_ = self.tokens.expect_static_string()?;
                    if notify_.eq_ignore_ascii_case("never") {
                        notify = Notify::Never;
                    } else {
                        let mut items = Vec::new();
                        for item in notify_.split(',') {
                            let item = item.trim();
                            if item.eq_ignore_ascii_case("success") {
                                items.push(NotifyItem::Success);
                            } else if item.eq_ignore_ascii_case("failure") {
                                items.push(NotifyItem::Failure);
                            } else if item.eq_ignore_ascii_case("delay") {
                                items.push(NotifyItem::Delay);
                            }
                        }
                        if !items.is_empty() {
                            notify = Notify::Items(items);
                        } else {
                            return Err(
                                token_info.expected("\"NEVER\" or \"SUCCESS, FAILURE, DELAY, ..\"")
                            );
                        }
                    }
                }
                _ => {
                    address = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.instructions.push(Instruction::Redirect(Redirect {
            address,
            copy,
            notify,
            return_of_content: ret,
            by_time: if let Some(alimit) = by_alimit {
                ByTime::Absolute {
                    alimit,
                    mode: by_mode,
                    trace: by_trace,
                }
            } else if let Some(rlimit) = by_rlimit {
                ByTime::Relative {
                    rlimit,
                    mode: by_mode,
                    trace: by_trace,
                }
            } else {
                ByTime::None
            },
            list,
        }));
        Ok(())
    }
}

impl MapLocalVars for ByTime<Value> {
    fn map_local_vars(&mut self, last_id: usize) {
        if let ByTime::Absolute { alimit, .. } = self {
            alimit.map_local_vars(last_id)
        }
    }
}
