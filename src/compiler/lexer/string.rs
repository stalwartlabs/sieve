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
    Envelope, MAX_MATCH_VARIABLES,
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
    EnvelopeVariable(Envelope),
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
                                    Some(("envelope", var_name)) if !var_name.is_empty() => {
                                        let envelope = match var_name {
                                            "from" => Envelope::From,
                                            "to" => Envelope::To,
                                            "by_time_absolute" => Envelope::ByTimeAbsolute,
                                            "by_time_relative" => Envelope::ByTimeRelative,
                                            "by_mode" => Envelope::ByMode,
                                            "by_trace" => Envelope::ByTrace,
                                            "notify" => Envelope::Notify,
                                            "orcpt" => Envelope::Orcpt,
                                            "ret" => Envelope::Ret,
                                            "envid" => Envelope::Envid,
                                            _ => {
                                                is_var_error = true;
                                                Envelope::From
                                            }
                                        };
                                        if !is_var_error {
                                            items.push(StringItem::EnvelopeVariable(envelope));
                                        }
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
            StringItem::LocalVariable(v) => write!(f, "${{{v}}}"),
            StringItem::MatchVariable(v) => write!(f, "${{{v}}}"),
            StringItem::GlobalVariable(v) => write!(f, "${{global.{v}}}"),
            StringItem::EnvironmentVariable(v) => write!(f, "${{env.{v}}}"),
            StringItem::List(l) => {
                for i in l {
                    i.fmt(f)?;
                }
                Ok(())
            }
            StringItem::EnvelopeVariable(env) => f.write_str(match env {
                Envelope::From => "${{envelope.from}}",
                Envelope::To => "${{envelope.to}}",
                Envelope::ByTimeAbsolute => "${{envelope.by_time_absolute}}",
                Envelope::ByTimeRelative => "${{envelope.by_time_relative}}",
                Envelope::ByMode => "${{envelope.by_mode}}",
                Envelope::ByTrace => "${{envelope.by_trace}}",
                Envelope::Notify => "${{envelope.notify}}",
                Envelope::Orcpt => "${{envelope.orcpt}}",
                Envelope::Ret => "${{envelope.ret}}",
                Envelope::Envid => "${{envelope.envit}}",
            }),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::StringItem;
    use crate::compiler::grammar::instruction::{Block, CompilerState, Instruction, MAX_PARAMS};
    use crate::compiler::grammar::test::Test;
    use crate::compiler::grammar::tests::test_string::TestString;
    use crate::compiler::grammar::{Comparator, MatchType};
    use crate::compiler::lexer::tokenizer::Tokenizer;
    use crate::compiler::lexer::word::Word;
    use crate::{AHashSet, Compiler};

    #[test]
    fn tokenize_string() {
        let c = Compiler::new();
        let mut block = Block::new(Word::Not);
        block.match_test_pos.push(0);
        let mut compiler = CompilerState {
            compiler: &c,
            instructions: vec![Instruction::Test(Test::String(TestString {
                match_type: MatchType::Regex(u64::MAX),
                comparator: Comparator::AsciiCaseMap,
                source: vec![StringItem::LocalVariable(0)],
                key_list: vec![StringItem::LocalVariable(0)],
                is_not: false,
            }))],
            block_stack: Vec::new(),
            block,
            last_block_type: Word::Not,
            vars_global: AHashSet::new(),
            vars_num: 0,
            vars_num_max: 0,
            tokens: Tokenizer::new(&c, b""),
            vars_match_max: usize::MAX,
            param_check: [false; MAX_PARAMS],
            includes_num: 0,
        };

        for (input, expected_result) in [
            ("$${hex:24 24}", StringItem::Text("$$$".to_string())),
            ("$${hex:40}", StringItem::Text("$@".to_string())),
            ("${hex: 40 }", StringItem::Text("@".to_string())),
            ("${HEX: 40}", StringItem::Text("@".to_string())),
            ("${hex:40", StringItem::Text("${hex:40".to_string())),
            ("${hex:400}", StringItem::Text("${hex:400}".to_string())),
            (
                "${hex:4${hex:30}}",
                StringItem::Text("${hex:40}".to_string()),
            ),
            ("${unicode:40}", StringItem::Text("@".to_string())),
            (
                "${ unicode:40}",
                StringItem::Text("${ unicode:40}".to_string()),
            ),
            ("${UNICODE:40}", StringItem::Text("@".to_string())),
            ("${UnICoDE:0000040}", StringItem::Text("@".to_string())),
            ("${Unicode:40}", StringItem::Text("@".to_string())),
            (
                "${Unicode:40 40 ",
                StringItem::Text("${Unicode:40 40 ".to_string()),
            ),
            (
                "${Unicode:Cool}",
                StringItem::Text("${Unicode:Cool}".to_string()),
            ),
            ("", StringItem::Text("".to_string())),
            (
                "${global.full}",
                StringItem::GlobalVariable("full".to_string()),
            ),
            (
                "${BAD${global.Company}",
                StringItem::List(vec![
                    StringItem::Text("${BAD".to_string()),
                    StringItem::GlobalVariable("company".to_string()),
                ]),
            ),
            (
                "${President, ${global.Company} Inc.}",
                StringItem::List(vec![
                    StringItem::Text("${President, ".to_string()),
                    StringItem::GlobalVariable("company".to_string()),
                    StringItem::Text(" Inc.}".to_string()),
                ]),
            ),
            (
                "dear${hex:20 24 7b}global.Name}",
                StringItem::List(vec![
                    StringItem::Text("dear ".to_string()),
                    StringItem::GlobalVariable("name".to_string()),
                ]),
            ),
            (
                "INBOX.lists.${2}",
                StringItem::List(vec![
                    StringItem::Text("INBOX.lists.".to_string()),
                    StringItem::MatchVariable(2),
                ]),
            ),
            (
                "Ein unerh${unicode:00F6}rt gro${unicode:00DF}er Test",
                StringItem::Text("Ein unerhört großer Test".to_string()),
            ),
            ("&%${}!", StringItem::Text("&%${}!".to_string())),
            ("${doh!}", StringItem::Text("${doh!}".to_string())),
            (
                "${hex: 20 }${global.hi}${hex: 20 }",
                StringItem::List(vec![
                    StringItem::Text(" ".to_string()),
                    StringItem::GlobalVariable("hi".to_string()),
                    StringItem::Text(" ".to_string()),
                ]),
            ),
            (
                "${hex:20 24 7b z}${global.hi}${unicode:}${unicode: }${hex:20}",
                StringItem::List(vec![
                    StringItem::Text("${hex:20 24 7b z}".to_string()),
                    StringItem::GlobalVariable("hi".to_string()),
                    StringItem::Text("${unicode:}${unicode: } ".to_string()),
                ]),
            ),
        ] {
            assert_eq!(
                compiler.tokenize_string(input.as_bytes(), true).unwrap(),
                expected_result,
                "Failed for {input}"
            );
        }

        for input in ["${unicode:200000}", "${Unicode:DF01}"] {
            assert!(compiler.tokenize_string(input.as_bytes(), true).is_err());
        }
    }
}
