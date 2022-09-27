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

#[cfg(test)]
mod tests {

    use crate::{runtime::StringItem, Compiler};

    #[test]
    fn tokenize_string() {
        let compiler = Compiler::new();

        for (input, expected_result) in [
            ("$${hex:24 24}", StringItem::Text(b"$$$".to_vec())),
            ("$${hex:40}", StringItem::Text(b"$@".to_vec())),
            ("${hex: 40 }", StringItem::Text(b"@".to_vec())),
            ("${HEX: 40}", StringItem::Text(b"@".to_vec())),
            ("${hex:40", StringItem::Text(b"${hex:40".to_vec())),
            ("${hex:400}", StringItem::Text(b"${hex:400}".to_vec())),
            ("${hex:4${hex:30}}", StringItem::Text(b"${hex:40}".to_vec())),
            ("${unicode:40}", StringItem::Text(b"@".to_vec())),
            (
                "${ unicode:40}",
                StringItem::Text(b"${ unicode:40}".to_vec()),
            ),
            ("${UNICODE:40}", StringItem::Text(b"@".to_vec())),
            ("${UnICoDE:0000040}", StringItem::Text(b"@".to_vec())),
            ("${Unicode:40}", StringItem::Text(b"@".to_vec())),
            (
                "${Unicode:40 40 ",
                StringItem::Text(b"${Unicode:40 40 ".to_vec()),
            ),
            (
                "${Unicode:Cool}",
                StringItem::Text(b"${Unicode:Cool}".to_vec()),
            ),
            ("", StringItem::Text(b"".to_vec())),
            ("${full}", StringItem::VariableName("full".to_string())),
            (
                "${BAD${Company}",
                StringItem::List(vec![
                    StringItem::Text(b"${BAD".to_vec()),
                    StringItem::VariableName("Company".to_string()),
                ]),
            ),
            (
                "${President, ${Company} Inc.}",
                StringItem::List(vec![
                    StringItem::Text(b"${President, ".to_vec()),
                    StringItem::VariableName("Company".to_string()),
                    StringItem::Text(b" Inc.}".to_vec()),
                ]),
            ),
            (
                "dear${hex:20 24 7b 4e}ame}",
                StringItem::List(vec![
                    StringItem::Text(b"dear ".to_vec()),
                    StringItem::VariableName("Name".to_string()),
                ]),
            ),
            (
                "INBOX.lists.${2}",
                StringItem::List(vec![
                    StringItem::Text(b"INBOX.lists.".to_vec()),
                    StringItem::VariableNumber(2),
                ]),
            ),
            (
                "Ein unerh${unicode:00F6}rt gro${unicode:00DF}er Test",
                StringItem::Text("Ein unerhört großer Test".to_string().into_bytes()),
            ),
            ("&%${}!", StringItem::Text(b"&%${}!".to_vec())),
            ("${doh!}", StringItem::Text(b"${doh!}".to_vec())),
            (
                "${hex: 20 }${hi}${hex: 20 }",
                StringItem::List(vec![
                    StringItem::Text(b" ".to_vec()),
                    StringItem::VariableName("hi".to_string()),
                    StringItem::Text(b" ".to_vec()),
                ]),
            ),
            (
                "${hex:20 24 7b z}${hi}${unicode:}${unicode: }${hex:20}",
                StringItem::List(vec![
                    StringItem::Text(b"${hex:20 24 7b z}".to_vec()),
                    StringItem::VariableName("hi".to_string()),
                    StringItem::Text(b"${unicode:}${unicode: } ".to_vec()),
                ]),
            ),
        ] {
            assert_eq!(
                compiler.tokenize_string(input.as_bytes(), true).unwrap(),
                expected_result,
                "Failed for {}",
                input
            );
        }

        for input in ["${unicode:200000}", "${Unicode:DF01}"] {
            assert!(compiler.tokenize_string(input.as_bytes(), true).is_err());
        }
    }
}
