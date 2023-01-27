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

use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::{
    compiler::{grammar::instruction::CompilerState, ErrorType},
    runtime::string::IntoString,
    MAX_MATCH_VARIABLES,
};

enum State {
    None,
    Variable,
    Encoded {
        is_unicode: bool,
        initial_buf_size: usize,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum StringItem {
    Text(String),
    LocalVariable(usize),
    MatchVariable(usize),
    GlobalVariable(String),
    EnvironmentVariable(String),
    List(Vec<StringItem>),
}

impl<'x> CompilerState<'x> {
    pub(crate) fn tokenize_string(
        &mut self,
        bytes: &[u8],
        parse_decoded: bool,
    ) -> Result<StringItem, ErrorType> {
        let mut state = State::None;
        let mut items = Vec::with_capacity(3);
        let mut last_ch = 0;

        let mut var_start_pos = usize::MAX;
        let mut var_is_number = true;
        let mut var_has_namespace = false;

        let mut hex_start = usize::MAX;
        let mut decode_buf = Vec::with_capacity(bytes.len());

        for (pos, &ch) in bytes.iter().enumerate() {
            let mut is_var_error = false;

            match state {
                State::None => {
                    if ch == b'{' && last_ch == b'$' {
                        decode_buf.pop();
                        var_start_pos = pos + 1;
                        var_is_number = true;
                        var_has_namespace = false;
                        state = State::Variable;
                    } else {
                        decode_buf.push(ch);
                    }
                }
                State::Variable => match ch {
                    b'a'..=b'z' | b'A'..=b'Z' | b'_' => {
                        var_is_number = false;
                    }
                    b'.' => {
                        var_is_number = false;
                        var_has_namespace = true;
                    }
                    b'0'..=b'9' => {}
                    b'}' => {
                        if pos > var_start_pos {
                            // Add any text before the variable
                            if !decode_buf.is_empty() {
                                self.add_string_item(&mut items, &decode_buf, parse_decoded)?;
                                decode_buf.clear();
                            }

                            if !var_has_namespace {
                                if !var_is_number {
                                    let var_name =
                                        String::from_utf8(bytes[var_start_pos..pos].to_vec())
                                            .unwrap();
                                    if self.is_var_global(&var_name) {
                                        items.push(StringItem::GlobalVariable(var_name));
                                    } else if let Some(var_id) = self.get_local_var(&var_name) {
                                        items.push(StringItem::LocalVariable(var_id));
                                    }
                                } else {
                                    let num_str =
                                        std::str::from_utf8(&bytes[var_start_pos..pos]).unwrap();
                                    let num = num_str.parse().map_err(|_| {
                                        ErrorType::InvalidNumber(num_str.to_string())
                                    })?;
                                    if num < MAX_MATCH_VARIABLES {
                                        if self.register_match_var(num) {
                                            let total_vars = num + 1;
                                            if total_vars > self.vars_match_max {
                                                self.vars_match_max = total_vars;
                                            }
                                            items.push(StringItem::MatchVariable(num));
                                        }
                                    } else {
                                        return Err(ErrorType::InvalidMatchVariable(num));
                                    }
                                }
                            } else {
                                match std::str::from_utf8(&bytes[var_start_pos..pos])
                                    .unwrap()
                                    .to_lowercase()
                                    .split_once('.')
                                {
                                    Some(("global", var_name)) if !var_name.is_empty() => {
                                        items
                                            .push(StringItem::GlobalVariable(var_name.to_string()));
                                    }
                                    Some(("env", var_name)) if !var_name.is_empty() => {
                                        items.push(StringItem::EnvironmentVariable(
                                            var_name.to_string(),
                                        ));
                                    }
                                    /*Some((namespace, _)) => {
                                        return Err(ErrorType::InvalidNamespace(
                                            namespace.to_string(),
                                        ));
                                    }*/
                                    _ => {
                                        is_var_error = true;
                                    }
                                }
                            }

                            state = State::None;
                        } else {
                            is_var_error = true;
                        }
                    }
                    b':' if parse_decoded => match bytes.get(var_start_pos..pos) {
                        Some(enc) if enc.eq_ignore_ascii_case(b"hex") => {
                            state = State::Encoded {
                                is_unicode: false,
                                initial_buf_size: decode_buf.len(),
                            };
                        }
                        Some(enc) if enc.eq_ignore_ascii_case(b"unicode") => {
                            state = State::Encoded {
                                is_unicode: true,
                                initial_buf_size: decode_buf.len(),
                            };
                        }
                        _ => {
                            is_var_error = true;
                        }
                    },
                    _ => {
                        is_var_error = true;
                    }
                },
                State::Encoded {
                    is_unicode,
                    initial_buf_size,
                } => match ch {
                    b'0'..=b'9' | b'a'..=b'f' | b'A'..=b'F' => {
                        if hex_start == usize::MAX {
                            hex_start = pos;
                        }
                    }
                    b' ' | b'\t' | b'\r' | b'\n' | b'}' => {
                        if hex_start != usize::MAX {
                            let code = std::str::from_utf8(&bytes[hex_start..pos]).unwrap();
                            hex_start = usize::MAX;

                            if !is_unicode {
                                if let Ok(ch) = u8::from_str_radix(code, 16) {
                                    decode_buf.push(ch);
                                } else {
                                    is_var_error = true;
                                }
                            } else if let Ok(ch) = u32::from_str_radix(code, 16) {
                                let mut buf = [0; 4];
                                decode_buf.extend_from_slice(
                                    char::from_u32(ch)
                                        .ok_or(ErrorType::InvalidUnicodeSequence(ch))?
                                        .encode_utf8(&mut buf)
                                        .as_bytes(),
                                );
                            } else {
                                is_var_error = true;
                            }
                        }
                        if ch == b'}' {
                            if decode_buf.len() != initial_buf_size {
                                state = State::None;
                            } else {
                                is_var_error = true;
                            }
                        }
                    }
                    _ => {
                        is_var_error = true;
                    }
                },
            }

            if is_var_error {
                if let State::Encoded {
                    initial_buf_size, ..
                } = state
                {
                    if initial_buf_size != decode_buf.len() {
                        decode_buf.truncate(initial_buf_size);
                    }
                }
                decode_buf.extend_from_slice(&bytes[var_start_pos - 2..pos + 1]);
                hex_start = usize::MAX;
                state = State::None;
            }

            last_ch = ch;
        }

        match state {
            State::Variable => {
                decode_buf.extend_from_slice(&bytes[var_start_pos - 2..bytes.len()]);
            }
            State::Encoded {
                initial_buf_size, ..
            } => {
                if initial_buf_size != decode_buf.len() {
                    decode_buf.truncate(initial_buf_size);
                }
                decode_buf.extend_from_slice(&bytes[var_start_pos - 2..bytes.len()]);
            }
            State::None => (),
        }

        if !decode_buf.is_empty() {
            self.add_string_item(&mut items, &decode_buf, parse_decoded)?;
        }

        Ok(match items.len() {
            1 => items.pop().unwrap(),
            0 => StringItem::Text(String::new()),
            _ => StringItem::List(items),
        })
    }

    #[inline(always)]
    fn add_string_item(
        &mut self,
        items: &mut Vec<StringItem>,
        buf: &[u8],
        parse_decoded: bool,
    ) -> Result<(), ErrorType> {
        if !parse_decoded {
            items.push(StringItem::Text(buf.to_vec().into_string()));
        } else {
            match self.tokenize_string(buf, false)? {
                StringItem::List(new_items) => items.extend(new_items),
                item => items.push(item),
            }
        }

        Ok(())
    }
}

impl Display for StringItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringItem::Text(t) => f.write_str(t),
            StringItem::LocalVariable(v) => write!(f, "${{{}}}", v),
            StringItem::MatchVariable(v) => write!(f, "${{{}}}", v),
            StringItem::GlobalVariable(v) => write!(f, "${{global.{}}}", v),
            StringItem::EnvironmentVariable(v) => write!(f, "${{env.{}}}", v),
            StringItem::List(l) => {
                for i in l {
                    i.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}
