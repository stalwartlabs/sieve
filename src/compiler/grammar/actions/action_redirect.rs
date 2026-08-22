/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    CompileError, Value,
    grammar::{
        Capability,
        instruction::{CompilerState, Instruction, MapLocalVars},
    },
    lexer::{Token, word::Word},
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct Redirect {
    pub copy: bool,
    pub address: Value,
    pub notify: Notify,
    pub return_of_content: Ret,
    pub by_time: ByTime<Value>,
    pub list: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub enum NotifyItem {
    Success = 0,
    Failure = 1,
    Delay = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub enum Notify {
    Never = 0,
    Items(Box<[NotifyItem]>) = 1,
    Default = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub enum Ret {
    Full = 0,
    Hdrs = 1,
    Default = 2,
}

/*

   Usage:   redirect [:bytimerelative <rlimit: number> /
                      :bytimeabsolute <alimit:string>
                      [:bymode "notify"|"return"] [:bytrace]]
                     <address: string>

*/

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub enum ByTime<T> {
    Relative {
        rlimit: u64,
        mode: ByMode,
        trace: bool,
    } = 0,
    Absolute {
        alimit: T,
        mode: ByMode,
        trace: bool,
    } = 1,
    None = 2,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub enum ByMode {
    Notify = 0,
    Return = 1,
    Default = 2,
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
                    if let Some(mode) = lookup_by_mode(&by_mode_) {
                        by_mode = mode;
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
                    if let Some(ret_of_content) = lookup_ret(&ret_) {
                        ret = ret_of_content;
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
                            if let Some(item) = lookup_notify_item(item.trim()) {
                                items.push(item);
                            }
                        }
                        if !items.is_empty() {
                            notify = Notify::Items(items.into());
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

        self.instructions
            .push(Instruction::Redirect(Box::new(Redirect {
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
            })));
        Ok(())
    }
}

impl MapLocalVars for ByTime<Value> {
    fn map_local_vars(&mut self, last_id: u16) {
        if let ByTime::Absolute { alimit, .. } = self {
            alimit.map_local_vars(last_id)
        }
    }
}

fn lookup_by_mode(input: &str) -> Option<ByMode> {
    hashify::tiny_map_ignore_case!(
        input.as_bytes(),
        "notify" => ByMode::Notify,
        "return" => ByMode::Return,
    )
}

fn lookup_ret(input: &str) -> Option<Ret> {
    hashify::tiny_map_ignore_case!(
        input.as_bytes(),
        "full" => Ret::Full,
        "hdrs" => Ret::Hdrs,
    )
}

fn lookup_notify_item(input: &str) -> Option<NotifyItem> {
    hashify::tiny_map_ignore_case!(
        input.as_bytes(),
        "success" => NotifyItem::Success,
        "failure" => NotifyItem::Failure,
        "delay" => NotifyItem::Delay,
    )
}
