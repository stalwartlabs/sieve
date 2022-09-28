use std::fmt::Display;

use crate::{compiler::ErrorType, runtime::StringItem, Compiler};

enum State {
    None,
    Variable,
    Encoded {
        is_unicode: bool,
        initial_buf_size: usize,
    },
}

impl Compiler {
    pub(crate) fn tokenize_string(
        &self,
        bytes: &[u8],
        parse_decoded: bool,
    ) -> Result<StringItem, ErrorType> {
        let mut state = State::None;
        let mut items = Vec::with_capacity(3);
        let mut last_ch = 0;

        let mut var_start_pos = usize::MAX;
        let mut var_is_number = true;

        let mut hex_start = usize::MAX;
        let mut decode_buf = Vec::with_capacity(bytes.len());
        let mut string_len = 0;

        for (pos, &ch) in bytes.iter().enumerate() {
            let mut is_var_error = false;

            match state {
                State::None => {
                    if ch == b'{' && last_ch == b'$' {
                        decode_buf.pop();
                        var_start_pos = pos + 1;
                        var_is_number = true;
                        state = State::Variable;
                    } else {
                        decode_buf.push(ch);
                    }
                }
                State::Variable => match ch {
                    b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'.' => {
                        var_is_number = false;
                    }
                    b'0'..=b'9' => {}
                    b'}' => {
                        if pos > var_start_pos {
                            // Add any text before the variable
                            if !decode_buf.is_empty() {
                                string_len += decode_buf.len();
                                self.add_string_item(&mut items, &decode_buf, parse_decoded)?;
                                decode_buf.clear();
                            }

                            items.push(if !var_is_number {
                                let var_len = pos - var_start_pos;
                                if var_len < self.max_variable_len {
                                    string_len += var_len;
                                    StringItem::VariableName(
                                        String::from_utf8(bytes[var_start_pos..pos].to_vec())
                                            .unwrap(),
                                    )
                                } else {
                                    return Err(ErrorType::VariableTooLong);
                                }
                            } else {
                                let num = std::str::from_utf8(&bytes[var_start_pos..pos]).unwrap();
                                StringItem::VariableNumber(
                                    num.parse()
                                        .map_err(|_| ErrorType::InvalidNumber(num.to_string()))?,
                                )
                            });
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
            string_len += decode_buf.len();
            self.add_string_item(&mut items, &decode_buf, parse_decoded)?;
        }

        if string_len < self.max_string_len {
            Ok(match items.len() {
                1 => items.pop().unwrap(),
                0 => StringItem::Text(Vec::new()),
                _ => StringItem::List(items),
            })
        } else {
            Err(ErrorType::StringTooLong)
        }
    }

    #[inline(always)]
    fn add_string_item(
        &self,
        items: &mut Vec<StringItem>,
        buf: &[u8],
        parse_decoded: bool,
    ) -> Result<(), ErrorType> {
        if !parse_decoded {
            items.push(StringItem::Text(buf.to_vec()));
        } else {
            match self.tokenize_string(buf, false)? {
                StringItem::List(new_items) => items.extend(new_items),
                item => items.push(item),
            }
        }

        Ok(())
    }
}

fn parse_matches(parts: &mut Vec<StringItem>, bytes: &[u8]) -> bool {
    let mut chars = Vec::with_capacity(bytes.len());
    let parts_start = parts.len();

    for &ch in bytes {
        match ch {
            b'*' => {
                if !chars.is_empty() {
                    parts.push(StringItem::Text(chars.to_vec()));
                    chars.clear();
                }
                if let Some(StringItem::MatchMany(count)) = parts.last_mut() {
                    *count += 1;
                } else {
                    parts.push(StringItem::MatchMany(1));
                }
            }
            b'?' => {
                if !chars.is_empty() {
                    parts.push(StringItem::Text(chars.to_vec()));
                    chars.clear();
                }
                parts.push(StringItem::MatchOne);
            }
            _ => {
                chars.push(ch);
            }
        }
    }

    let has_matches = parts_start != parts.len();
    if has_matches && !chars.is_empty() {
        parts.push(StringItem::Text(chars.to_vec()));
    }
    has_matches
}

impl StringItem {
    pub fn into_matches(self) -> Self {
        match self {
            StringItem::Text(text) => {
                let mut list = Vec::new();
                if parse_matches(&mut list, &text) {
                    StringItem::List(list)
                } else {
                    StringItem::Text(text)
                }
            }
            StringItem::List(list) => {
                let mut new_list = Vec::with_capacity(list.len());
                for item in list {
                    match item {
                        StringItem::Text(text) => {
                            if !parse_matches(&mut new_list, &text) {
                                new_list.push(StringItem::Text(text));
                            }
                        }
                        _ => new_list.push(item),
                    }
                }
                StringItem::List(new_list)
            }
            _ => self,
        }
    }
}

impl Display for StringItem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StringItem::Text(t) => f.write_str(&String::from_utf8_lossy(t)),
            StringItem::VariableName(v) => write!(f, "${{{}}}", v),
            StringItem::VariableNumber(v) => write!(f, "${{{}}}", v),
            StringItem::MatchMany(_) => f.write_str("*"),
            StringItem::MatchOne => f.write_str("?"),
            StringItem::List(l) => {
                for i in l {
                    i.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}
