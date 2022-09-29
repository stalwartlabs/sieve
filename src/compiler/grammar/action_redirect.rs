use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError,
    },
    runtime::StringItem,
};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Redirect {
    pub copy: bool,
    pub address: StringItem,
    pub notify: NotifyValue,
    pub ret: Ret,
    pub by_time: ByTime,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NotifyItem {
    Success,
    Failure,
    Delay,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NotifyValue {
    Never,
    Items(Vec<NotifyItem>),
    Default,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Ret {
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

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ByTime {
    Relative {
        rlimit: u64,
        mode: ByMode,
        trace: bool,
    },
    Absolute {
        alimit: StringItem,
        mode: ByMode,
        trace: bool,
    },
    None,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ByMode {
    Notify,
    Return,
    Default,
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_redirect(&mut self) -> Result<Redirect, CompileError> {
        let mut address = None;
        let mut copy = false;
        let mut ret = Ret::Default;
        let mut notify = NotifyValue::Default;
        let mut by_mode = ByMode::Default;
        let mut by_trace = false;
        let mut by_rlimit = None;
        let mut by_alimit = None;

        while address.is_none() {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Copy) => {
                    copy = true;
                }
                Token::Tag(Word::ByTrace) => {
                    by_trace = true;
                }
                Token::Tag(Word::ByMode) => {
                    let by_mode_ = self.unwrap_static_string()?;
                    if by_mode_.eq_ignore_ascii_case(b"notify") {
                        by_mode = ByMode::Notify;
                    } else if by_mode_.eq_ignore_ascii_case(b"return") {
                        by_mode = ByMode::Return;
                    } else {
                        return Err(token_info.expected("\"notify\" or \"return\""));
                    }
                }
                Token::Tag(Word::ByTimeRelative) => {
                    by_rlimit = (self.unwrap_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::ByTimeAbsolute) => {
                    by_alimit = self.unwrap_string()?.into();
                }
                Token::Tag(Word::Ret) => {
                    let ret_ = self.unwrap_static_string()?;
                    if ret_.eq_ignore_ascii_case(b"full") {
                        ret = Ret::Full;
                    } else if ret_.eq_ignore_ascii_case(b"hdrs") {
                        ret = Ret::Hdrs;
                    } else {
                        return Err(token_info.expected("\"FULL\" or \"HDRS\""));
                    }
                }
                Token::Tag(Word::Notify) => {
                    let notify_ = self.unwrap_static_string()?;
                    if notify_.eq_ignore_ascii_case(b"never") {
                        notify = NotifyValue::Never;
                    } else {
                        let mut items = Vec::new();
                        for item in String::from_utf8_lossy(&notify_).split(',') {
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
                            notify = NotifyValue::Items(items);
                        } else {
                            return Err(
                                token_info.expected("\"NEVER\" or \"SUCCESS, FAILURE, DELAY, ..\"")
                            );
                        }
                    }
                }
                Token::String(string) => {
                    address = string.into();
                }
                _ => {
                    return Err(token_info.expected("string"));
                }
            }
        }

        Ok(Redirect {
            address: address.unwrap(),
            copy,
            notify,
            ret,
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
        })
    }
}
