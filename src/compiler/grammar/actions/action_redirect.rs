use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Redirect {
    pub copy: bool,
    pub address: StringItem,
    pub notify: NotifyValue,
    pub ret: Ret,
    pub by_time: ByTime,
    pub list: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NotifyItem {
    Success,
    Failure,
    Delay,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum NotifyValue {
    Never,
    Items(Vec<NotifyItem>),
    Default,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ByMode {
    Notify,
    Return,
    Default,
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_redirect(&mut self) -> Result<(), CompileError> {
        let address;
        let mut copy = false;
        let mut ret = Ret::Default;
        let mut notify = NotifyValue::Default;
        let mut list = false;
        let mut by_mode = ByMode::Default;
        let mut by_trace = false;
        let mut by_rlimit = None;
        let mut by_alimit = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(Word::Copy) => {
                    copy = true;
                }
                Token::Tag(Word::List) => {
                    list = true;
                }
                Token::Tag(Word::ByTrace) => {
                    by_trace = true;
                }
                Token::Tag(Word::ByMode) => {
                    let by_mode_ = self.tokens.expect_static_string()?;
                    if by_mode_.eq_ignore_ascii_case(b"notify") {
                        by_mode = ByMode::Notify;
                    } else if by_mode_.eq_ignore_ascii_case(b"return") {
                        by_mode = ByMode::Return;
                    } else {
                        return Err(token_info.expected("\"notify\" or \"return\""));
                    }
                }
                Token::Tag(Word::ByTimeRelative) => {
                    by_rlimit = (self.tokens.expect_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::ByTimeAbsolute) => {
                    by_alimit = self.parse_string()?.into();
                }
                Token::Tag(Word::Ret) => {
                    let ret_ = self.tokens.expect_static_string()?;
                    if ret_.eq_ignore_ascii_case(b"full") {
                        ret = Ret::Full;
                    } else if ret_.eq_ignore_ascii_case(b"hdrs") {
                        ret = Ret::Hdrs;
                    } else {
                        return Err(token_info.expected("\"FULL\" or \"HDRS\""));
                    }
                }
                Token::Tag(Word::Notify) => {
                    let notify_ = self.tokens.expect_static_string()?;
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
                _ => {
                    address = self.parse_string_token(token_info)?;
                    break;
                }
            }
        }

        self.commands.push(Command::Redirect(Redirect {
            address,
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
            list,
        }));
        Ok(())
    }
}
