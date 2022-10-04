use std::fmt::Display;

use serde::{Deserialize, Serialize};

use crate::compiler::{grammar::command::CompilerState, ErrorType};

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
    Text(Vec<u8>),
    LocalVariable(usize),
    MatchVariable(usize),
    GlobalVariable(String),
    MatchMany(usize),
    MatchOne,
    List(Vec<StringItem>),
}

impl<'x> CompilerState<'x> {
    pub(crate) fn tokenize_string(
        &self,
        bytes: &[u8],
        parse_decoded: bool,
        parse_matches: bool,
    ) -> Result<StringItem, ErrorType> {
        let mut state = State::None;
        let mut items = Vec::with_capacity(3);
        let mut last_ch = 0;

        let mut var_start_pos = usize::MAX;
        let mut var_is_number = true;

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
                                self.add_string_item(
                                    &mut items,
                                    &decode_buf,
                                    parse_decoded,
                                    parse_matches,
                                )?;
                                decode_buf.clear();
                            }

                            if !var_is_number {
                                if pos - var_start_pos > 7
                                    && bytes[var_start_pos..var_start_pos + 7]
                                        .eq_ignore_ascii_case(b"global.")
                                {
                                    items.push(StringItem::GlobalVariable(
                                        String::from_utf8(bytes[var_start_pos + 7..pos].to_vec())
                                            .unwrap(),
                                    ));
                                } else {
                                    let var_name =
                                        String::from_utf8(bytes[var_start_pos..pos].to_vec())
                                            .unwrap();
                                    if self.is_var_global(&var_name) {
                                        items.push(StringItem::GlobalVariable(var_name));
                                    } else if let Some(var_id) = self.get_local_var(&var_name) {
                                        items.push(StringItem::LocalVariable(var_id));
                                    }
                                }
                            } else {
                                let num = std::str::from_utf8(&bytes[var_start_pos..pos]).unwrap();
                                items.push(StringItem::MatchVariable(
                                    num.parse()
                                        .map_err(|_| ErrorType::InvalidNumber(num.to_string()))?,
                                ));
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
            self.add_string_item(&mut items, &decode_buf, parse_decoded, parse_matches)?;
        }

        Ok(match items.len() {
            1 => items.pop().unwrap(),
            0 => StringItem::Text(Vec::new()),
            _ => StringItem::List(items),
        })
    }

    #[inline(always)]
    fn add_string_item(
        &self,
        items: &mut Vec<StringItem>,
        buf: &[u8],
        parse_decoded: bool,
        parse_matches: bool,
    ) -> Result<(), ErrorType> {
        if !parse_decoded {
            if !parse_matches {
                items.push(StringItem::Text(buf.to_vec()));
            } else {
                let mut chars = Vec::with_capacity(buf.len());
                let items_start = items.len();

                for &ch in buf {
                    match ch {
                        b'*' => {
                            if !chars.is_empty() {
                                items.push(StringItem::Text(chars.to_vec()));
                                chars.clear();
                            }
                            if let Some(StringItem::MatchMany(count)) = items.last_mut() {
                                *count += 1;
                            } else {
                                items.push(StringItem::MatchMany(1));
                            }
                        }
                        b'?' => {
                            if !chars.is_empty() {
                                items.push(StringItem::Text(chars.to_vec()));
                                chars.clear();
                            }
                            items.push(StringItem::MatchOne);
                        }
                        _ => {
                            chars.push(ch);
                        }
                    }
                }

                if items_start != items.len() && !chars.is_empty() {
                    items.push(StringItem::Text(chars.to_vec()));
                }
            }
        } else {
            match self.tokenize_string(buf, false, parse_matches)? {
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
            StringItem::LocalVariable(v) => write!(f, "${{{}}}", v),
            StringItem::MatchVariable(v) => write!(f, "${{{}}}", v),
            StringItem::GlobalVariable(v) => write!(f, "${{global.{}}}", v),
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

    use super::StringItem;
    use crate::compiler::grammar::command::{Block, CompilerState};
    use crate::compiler::lexer::tokenizer::Tokenizer;
    use crate::compiler::lexer::word::Word;
    use crate::{AHashSet, Compiler};

    #[test]
    fn tokenize_string() {
        let c = Compiler::new();
        let compiler = CompilerState {
            commands: Vec::new(),
            block_stack: Vec::new(),
            block: Block::new(Word::Not),
            last_block_type: Word::Not,
            vars_global: AHashSet::new(),
            vars_num: 0,
            tokens: Tokenizer::new(&c, b""),
        };

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
            (
                "${global.full}",
                StringItem::GlobalVariable("full".to_string()),
            ),
            (
                "${BAD${global.Company}",
                StringItem::List(vec![
                    StringItem::Text(b"${BAD".to_vec()),
                    StringItem::GlobalVariable("Company".to_string()),
                ]),
            ),
            (
                "${President, ${global.Company} Inc.}",
                StringItem::List(vec![
                    StringItem::Text(b"${President, ".to_vec()),
                    StringItem::GlobalVariable("Company".to_string()),
                    StringItem::Text(b" Inc.}".to_vec()),
                ]),
            ),
            (
                "dear${hex:20 24 7b}global.Name}",
                StringItem::List(vec![
                    StringItem::Text(b"dear ".to_vec()),
                    StringItem::GlobalVariable("Name".to_string()),
                ]),
            ),
            (
                "INBOX.lists.${2}",
                StringItem::List(vec![
                    StringItem::Text(b"INBOX.lists.".to_vec()),
                    StringItem::MatchVariable(2),
                ]),
            ),
            (
                "Ein unerh${unicode:00F6}rt gro${unicode:00DF}er Test",
                StringItem::Text("Ein unerhört großer Test".to_string().into_bytes()),
            ),
            ("&%${}!", StringItem::Text(b"&%${}!".to_vec())),
            ("${doh!}", StringItem::Text(b"${doh!}".to_vec())),
            (
                "${hex: 20 }${global.hi}${hex: 20 }",
                StringItem::List(vec![
                    StringItem::Text(b" ".to_vec()),
                    StringItem::GlobalVariable("hi".to_string()),
                    StringItem::Text(b" ".to_vec()),
                ]),
            ),
            (
                "${hex:20 24 7b z}${global.hi}${unicode:}${unicode: }${hex:20}",
                StringItem::List(vec![
                    StringItem::Text(b"${hex:20 24 7b z}".to_vec()),
                    StringItem::GlobalVariable("hi".to_string()),
                    StringItem::Text(b"${unicode:}${unicode: } ".to_vec()),
                ]),
            ),
        ] {
            assert_eq!(
                compiler
                    .tokenize_string(input.as_bytes(), true, false)
                    .unwrap(),
                expected_result,
                "Failed for {}",
                input
            );
        }

        for input in ["${unicode:200000}", "${Unicode:DF01}"] {
            assert!(compiler
                .tokenize_string(input.as_bytes(), true, false)
                .is_err());
        }
    }
}
