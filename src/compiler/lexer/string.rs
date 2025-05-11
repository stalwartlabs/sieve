/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::fmt::Display;

use mail_parser::HeaderName;

use crate::{
    compiler::{
        grammar::{
            expr::{self},
            instruction::CompilerState,
            AddressPart,
        },
        ContentTypePart, ErrorType, HeaderPart, HeaderVariable, MessagePart, Number,
        ReceivedHostname, ReceivedPart, Value, VariableType,
    },
    runtime::eval::IntoString,
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

impl CompilerState<'_> {
    pub(crate) fn tokenize_string(
        &mut self,
        bytes: &[u8],
        parse_decoded: bool,
    ) -> Result<Value, ErrorType> {
        let mut state = State::None;
        let mut items = Vec::with_capacity(3);
        let mut last_ch = 0;

        let mut var_start_pos = usize::MAX;
        let mut var_is_number = true;
        let mut var_has_namespace = false;

        let mut text_has_digits = true;
        let mut text_has_dots = false;

        let mut hex_start = usize::MAX;
        let mut decode_buf = Vec::with_capacity(bytes.len());

        for (pos, &ch) in bytes.iter().enumerate() {
            let mut is_var_error = false;

            match state {
                State::None => match ch {
                    b'{' if last_ch == b'$' => {
                        decode_buf.pop();
                        var_start_pos = pos + 1;
                        var_is_number = true;
                        var_has_namespace = false;
                        state = State::Variable;
                    }
                    b'.' => {
                        if text_has_dots {
                            text_has_digits = false;
                        } else {
                            text_has_dots = true;
                        }
                        decode_buf.push(ch);
                    }
                    b'0'..=b'9' => {
                        decode_buf.push(ch);
                    }
                    _ => {
                        text_has_digits = false;
                        decode_buf.push(ch);
                    }
                },
                State::Variable => match ch {
                    b'a'..=b'z' | b'A'..=b'Z' | b'_' | b'[' | b']' | b'*' | b'-' => {
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
                                self.add_value(
                                    &mut items,
                                    &decode_buf,
                                    parse_decoded,
                                    text_has_digits,
                                    text_has_dots,
                                )?;
                                decode_buf.clear();
                                text_has_digits = true;
                                text_has_dots = false;
                            }

                            // Parse variable type
                            let var_name = std::str::from_utf8(&bytes[var_start_pos..pos]).unwrap();
                            let var_type = if !var_is_number {
                                self.parse_variable(var_name, var_has_namespace)
                            } else {
                                self.parse_match_variable(var_name)
                            };

                            match var_type {
                                Ok(Some(var)) => items.push(Value::Variable(var)),
                                Ok(None) => {}
                                Err(
                                    ErrorType::InvalidNamespace(_) | ErrorType::InvalidEnvelope(_),
                                ) => {
                                    is_var_error = true;
                                }
                                Err(e) => return Err(e),
                            }

                            state = State::None;
                        } else {
                            is_var_error = true;
                        }
                    }
                    b':' => {
                        if parse_decoded && !var_has_namespace {
                            match bytes.get(var_start_pos..pos) {
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
                            }
                        } else if var_has_namespace {
                            var_is_number = false;
                        } else {
                            is_var_error = true;
                        }
                    }
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
            self.add_value(
                &mut items,
                &decode_buf,
                parse_decoded,
                text_has_digits,
                text_has_dots,
            )?;
        }

        Ok(match items.len() {
            1 => items.pop().unwrap(),
            0 => Value::Text(String::new().into()),
            _ => Value::List(items),
        })
    }

    fn parse_match_variable(&mut self, var_name: &str) -> Result<Option<VariableType>, ErrorType> {
        let num = var_name
            .parse()
            .map_err(|_| ErrorType::InvalidNumber(var_name.to_string()))?;
        if num < MAX_MATCH_VARIABLES as usize {
            if self.register_match_var(num) {
                let total_vars = num + 1;
                if total_vars > self.vars_match_max {
                    self.vars_match_max = total_vars;
                }
                Ok(Some(VariableType::Match(num)))
            } else {
                Ok(None)
            }
        } else {
            Err(ErrorType::InvalidMatchVariable(num))
        }
    }

    pub fn parse_variable(
        &self,
        var_name: &str,
        maybe_namespace: bool,
    ) -> Result<Option<VariableType>, ErrorType> {
        if !maybe_namespace {
            if self.is_var_global(var_name) {
                Ok(Some(VariableType::Global(var_name.to_string())))
            } else if let Some(var_id) = self.get_local_var(var_name) {
                Ok(Some(VariableType::Local(var_id)))
            } else {
                Ok(None)
            }
        } else {
            let var = match var_name.to_lowercase().split_once('.') {
                Some(("global" | "t", var_name)) if !var_name.is_empty() => {
                    VariableType::Global(var_name.to_string())
                }
                Some(("env", var_name)) if !var_name.is_empty() => {
                    VariableType::Environment(var_name.to_string())
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
                            return Err(ErrorType::InvalidEnvelope(var_name.to_string()));
                        }
                    };
                    VariableType::Envelope(envelope)
                }
                Some(("header", var_name)) if !var_name.is_empty() => {
                    self.parse_header_variable(var_name)?
                }
                Some(("body", var_name)) if !var_name.is_empty() => match var_name {
                    "text" => VariableType::Part(MessagePart::TextBody(false)),
                    "html" => VariableType::Part(MessagePart::HtmlBody(false)),
                    "to_text" => VariableType::Part(MessagePart::TextBody(true)),
                    "to_html" => VariableType::Part(MessagePart::HtmlBody(true)),
                    _ => return Err(ErrorType::InvalidNamespace(var_name.to_string())),
                },
                Some(("part", var_name)) if !var_name.is_empty() => match var_name {
                    "text" => VariableType::Part(MessagePart::Contents),
                    "raw" => VariableType::Part(MessagePart::Raw),
                    _ => return Err(ErrorType::InvalidNamespace(var_name.to_string())),
                },
                None => {
                    if self.is_var_global(var_name) {
                        VariableType::Global(var_name.to_string())
                    } else if let Some(var_id) = self.get_local_var(var_name) {
                        VariableType::Local(var_id)
                    } else {
                        return Ok(None);
                    }
                }
                _ => return Err(ErrorType::InvalidNamespace(var_name.to_string())),
            };

            Ok(Some(var))
        }
    }

    fn parse_header_variable(&self, var_name: &str) -> Result<VariableType, ErrorType> {
        #[derive(Debug)]
        enum State {
            Name,
            Index,
            Part,
            PartIndex,
        }
        let mut name = vec![];
        let mut has_name = false;
        let mut has_wildcard = false;
        let mut hdr_name = String::new();
        let mut hdr_index = String::new();
        let mut part = String::new();
        let mut part_index = String::new();
        let mut state = State::Name;

        for ch in var_name.chars() {
            match state {
                State::Name => match ch {
                    '[' => {
                        state = if hdr_index.is_empty() {
                            State::Index
                        } else if part.is_empty() {
                            State::PartIndex
                        } else {
                            return Err(ErrorType::InvalidExpression(var_name.to_string()));
                        };
                        has_name = true;
                    }
                    '.' => {
                        state = State::Part;
                        has_name = true;
                    }
                    ' ' | '\t' | '\r' | '\n' => {}
                    '*' if !has_wildcard && hdr_name.is_empty() && name.is_empty() => {
                        has_wildcard = true;
                    }
                    ':' if !hdr_name.is_empty() && !has_wildcard => {
                        name.push(
                            HeaderName::parse(std::mem::take(&mut hdr_name)).ok_or_else(|| {
                                ErrorType::InvalidExpression(var_name.to_string())
                            })?,
                        );
                    }
                    _ if !has_name && !has_wildcard => {
                        hdr_name.push(ch);
                    }
                    _ => {
                        return Err(ErrorType::InvalidExpression(var_name.to_string()));
                    }
                },
                State::Index => match ch {
                    ']' => {
                        state = State::Name;
                    }
                    ' ' | '\t' | '\r' | '\n' => {}
                    _ => {
                        hdr_index.push(ch);
                    }
                },
                State::Part => match ch {
                    '[' => {
                        state = State::PartIndex;
                    }
                    ' ' | '\t' | '\r' | '\n' => {}
                    _ => {
                        part.push(ch);
                    }
                },
                State::PartIndex => match ch {
                    ']' => {
                        state = State::Name;
                    }
                    ' ' | '\t' | '\r' | '\n' => {}
                    _ => {
                        part_index.push(ch);
                    }
                },
            }
        }

        if !hdr_name.is_empty() {
            name.push(
                HeaderName::parse(hdr_name)
                    .ok_or_else(|| ErrorType::InvalidExpression(var_name.to_string()))?,
            );
        }

        if !name.is_empty() || has_wildcard {
            Ok(VariableType::Header(HeaderVariable {
                name,
                part: HeaderPart::try_from(part.as_str())
                    .map_err(|_| ErrorType::InvalidExpression(var_name.to_string()))?,
                index_hdr: match hdr_index.as_str() {
                    "" => {
                        if !has_wildcard {
                            -1
                        } else {
                            0
                        }
                    }
                    "*" => 0,
                    _ => hdr_index
                        .parse()
                        .map(|v| if v == 0 { 1 } else { v })
                        .map_err(|_| ErrorType::InvalidExpression(var_name.to_string()))?,
                },
                index_part: match part_index.as_str() {
                    "" => {
                        if !has_wildcard {
                            -1
                        } else {
                            0
                        }
                    }
                    "*" => 0,
                    _ => part_index
                        .parse()
                        .map(|v| if v == 0 { 1 } else { v })
                        .map_err(|_| ErrorType::InvalidExpression(var_name.to_string()))?,
                },
            }))
        } else {
            Err(ErrorType::InvalidExpression(var_name.to_string()))
        }
    }

    pub fn parse_expr_fnc_or_var(
        &self,
        var_name: &str,
        maybe_namespace: bool,
    ) -> Result<expr::Token, String> {
        match self.parse_variable(var_name, maybe_namespace) {
            Ok(Some(var)) => Ok(expr::Token::Variable(var)),
            _ => {
                if let Some((id, num_args)) = self.compiler.functions.get(var_name) {
                    Ok(expr::Token::Function {
                        name: var_name.to_string(),
                        id: *id,
                        num_args: *num_args,
                    })
                } else {
                    Err(format!("Invalid variable or function name {var_name:?}"))
                }
            }
        }
    }

    #[inline(always)]
    fn add_value(
        &mut self,
        items: &mut Vec<Value>,
        buf: &[u8],
        parse_decoded: bool,
        has_digits: bool,
        has_dots: bool,
    ) -> Result<(), ErrorType> {
        if !parse_decoded {
            items.push(if has_digits {
                if has_dots {
                    match std::str::from_utf8(buf)
                        .ok()
                        .and_then(|v| (v, v.parse::<f64>().ok()?).into())
                    {
                        Some((v, n)) if n.to_string() == v => Value::Number(Number::Float(n)),
                        _ => Value::Text(buf.to_vec().into_string().into()),
                    }
                } else {
                    match std::str::from_utf8(buf)
                        .ok()
                        .and_then(|v| (v, v.parse::<i64>().ok()?).into())
                    {
                        Some((v, n)) if n.to_string() == v => Value::Number(Number::Integer(n)),
                        _ => Value::Text(buf.to_vec().into_string().into()),
                    }
                }
            } else {
                Value::Text(buf.to_vec().into_string().into())
            });
        } else {
            match self.tokenize_string(buf, false)? {
                Value::List(new_items) => items.extend(new_items),
                item => items.push(item),
            }
        }

        Ok(())
    }
}

impl TryFrom<&str> for HeaderPart {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        let (value, subvalue) = value.split_once('.').unwrap_or((value, ""));
        Ok(match value {
            "" | "text" => HeaderPart::Text,
            // Addresses
            "name" => HeaderPart::Address(AddressPart::Name),
            "addr" => {
                if !subvalue.is_empty() {
                    HeaderPart::Address(AddressPart::try_from(subvalue)?)
                } else {
                    HeaderPart::Address(AddressPart::All)
                }
            }

            // Content-type
            "type" => HeaderPart::ContentType(ContentTypePart::Type),
            "subtype" => HeaderPart::ContentType(ContentTypePart::Subtype),
            "attr" if !subvalue.is_empty() => {
                HeaderPart::ContentType(ContentTypePart::Attribute(subvalue.to_string()))
            }

            // Received
            "rcvd" => {
                if !subvalue.is_empty() {
                    HeaderPart::Received(ReceivedPart::try_from(subvalue)?)
                } else {
                    HeaderPart::Text
                }
            }

            // Id
            "id" => HeaderPart::Id,

            // Raw
            "raw" => HeaderPart::Raw,
            "raw_name" => HeaderPart::RawName,

            // Date
            "date" => HeaderPart::Date,

            // Exists
            "exists" => HeaderPart::Exists,

            _ => {
                return Err(());
            }
        })
    }
}

impl TryFrom<&str> for ReceivedPart {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            // Received
            "from" => ReceivedPart::From(ReceivedHostname::Any),
            "from.name" => ReceivedPart::From(ReceivedHostname::Name),
            "from.ip" => ReceivedPart::From(ReceivedHostname::Ip),
            "ip" => ReceivedPart::FromIp,
            "iprev" => ReceivedPart::FromIpRev,
            "by" => ReceivedPart::By(ReceivedHostname::Any),
            "by.name" => ReceivedPart::By(ReceivedHostname::Name),
            "by.ip" => ReceivedPart::By(ReceivedHostname::Ip),
            "for" => ReceivedPart::For,
            "with" => ReceivedPart::With,
            "tls" => ReceivedPart::TlsVersion,
            "cipher" => ReceivedPart::TlsCipher,
            "id" => ReceivedPart::Id,
            "ident" => ReceivedPart::Ident,
            "date" => ReceivedPart::Date,
            "date.raw" => ReceivedPart::DateRaw,
            _ => return Err(()),
        })
    }
}

impl TryFrom<&str> for AddressPart {
    type Error = ();

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Ok(match value {
            "name" => AddressPart::Name,
            "addr" | "all" => AddressPart::All,
            "addr.domain" => AddressPart::Domain,
            "addr.local" => AddressPart::LocalPart,
            "addr.user" => AddressPart::User,
            "addr.detail" => AddressPart::Detail,
            _ => return Err(()),
        })
    }
}

impl Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::Text(t) => f.write_str(t),
            Value::List(l) => {
                for i in l {
                    i.fmt(f)?;
                }
                Ok(())
            }
            Value::Number(n) => n.fmt(f),
            Value::Variable(v) => v.fmt(f),
            Value::Regex(r) => f.write_str(&r.expr),
        }
    }
}

impl Display for VariableType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VariableType::Local(v) => write!(f, "${{{v}}}"),
            VariableType::Match(v) => write!(f, "${{{v}}}"),
            VariableType::Global(v) => write!(f, "${{global.{v}}}"),
            VariableType::Environment(v) => write!(f, "${{env.{v}}}"),

            VariableType::Envelope(env) => f.write_str(match env {
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

            VariableType::Header(hdr) => {
                write!(
                    f,
                    "${{header.{}",
                    hdr.name.first().map(|h| h.as_str()).unwrap_or_default()
                )?;
                if hdr.index_hdr != 0 {
                    write!(f, "[{}]", hdr.index_hdr)?;
                } else {
                    f.write_str("[*]")?;
                }
                /*if hdr.part != HeaderPart::Text {
                    f.write_str(".")?;
                    f.write_str(match &hdr.part {
                        HeaderPart::Name => "name",
                        HeaderPart::Address => "address",
                        HeaderPart::Type => "type",
                        HeaderPart::Subtype => "subtype",
                        HeaderPart::Raw => "raw",
                        HeaderPart::Date => "date",
                        HeaderPart::Attribute(attr) => attr.as_str(),
                        HeaderPart::Text => unreachable!(),
                    })?;
                }*/
                if hdr.index_part != 0 {
                    write!(f, "[{}]", hdr.index_part)?;
                } else {
                    f.write_str("[*]")?;
                }
                f.write_str("}")
            }
            VariableType::Part(part) => {
                write!(
                    f,
                    "${{{}",
                    match part {
                        MessagePart::TextBody(true) => "body.to_text",
                        MessagePart::TextBody(false) => "body.text",
                        MessagePart::HtmlBody(true) => "body.to_html",
                        MessagePart::HtmlBody(false) => "body.html",
                        MessagePart::Contents => "part.text",
                        MessagePart::Raw => "part.raw",
                    }
                )?;
                f.write_str("}")
            }
        }
    }
}

#[cfg(test)]
mod tests {

    use mail_parser::HeaderName;

    use super::Value;
    use crate::compiler::grammar::instruction::{Block, CompilerState, Instruction, MAX_PARAMS};
    use crate::compiler::grammar::test::Test;
    use crate::compiler::grammar::tests::test_string::TestString;
    use crate::compiler::grammar::{Comparator, MatchType};
    use crate::compiler::lexer::tokenizer::Tokenizer;
    use crate::compiler::lexer::word::Word;
    use crate::compiler::{AddressPart, HeaderPart, HeaderVariable, VariableType};
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
                source: vec![Value::Variable(VariableType::Local(0))],
                key_list: vec![Value::Variable(VariableType::Local(0))],
                is_not: false,
            }))],
            block_stack: Vec::new(),
            block,
            last_block_type: Word::Not,
            vars_global: AHashSet::new(),
            vars_num: 0,
            vars_num_max: 0,
            vars_local: 0,
            tokens: Tokenizer::new(&c, b""),
            vars_match_max: usize::MAX,
            param_check: [false; MAX_PARAMS],
            includes_num: 0,
        };

        for (input, expected_result) in [
            ("$${hex:24 24}", Value::Text("$$$".to_string().into())),
            ("$${hex:40}", Value::Text("$@".to_string().into())),
            ("${hex: 40 }", Value::Text("@".to_string().into())),
            ("${HEX: 40}", Value::Text("@".to_string().into())),
            ("${hex:40", Value::Text("${hex:40".to_string().into())),
            ("${hex:400}", Value::Text("${hex:400}".to_string().into())),
            (
                "${hex:4${hex:30}}",
                Value::Text("${hex:40}".to_string().into()),
            ),
            ("${unicode:40}", Value::Text("@".to_string().into())),
            (
                "${ unicode:40}",
                Value::Text("${ unicode:40}".to_string().into()),
            ),
            ("${UNICODE:40}", Value::Text("@".to_string().into())),
            ("${UnICoDE:0000040}", Value::Text("@".to_string().into())),
            ("${Unicode:40}", Value::Text("@".to_string().into())),
            (
                "${Unicode:40 40 ",
                Value::Text("${Unicode:40 40 ".to_string().into()),
            ),
            (
                "${Unicode:Cool}",
                Value::Text("${Unicode:Cool}".to_string().into()),
            ),
            ("", Value::Text("".to_string().into())),
            (
                "${global.full}",
                Value::Variable(VariableType::Global("full".to_string())),
            ),
            (
                "${BAD${global.Company}",
                Value::List(vec![
                    Value::Text("${BAD".to_string().into()),
                    Value::Variable(VariableType::Global("company".to_string())),
                ]),
            ),
            (
                "${President, ${global.Company} Inc.}",
                Value::List(vec![
                    Value::Text("${President, ".to_string().into()),
                    Value::Variable(VariableType::Global("company".to_string())),
                    Value::Text(" Inc.}".to_string().into()),
                ]),
            ),
            (
                "dear${hex:20 24 7b}global.Name}",
                Value::List(vec![
                    Value::Text("dear ".to_string().into()),
                    Value::Variable(VariableType::Global("name".to_string())),
                ]),
            ),
            (
                "INBOX.lists.${2}",
                Value::List(vec![
                    Value::Text("INBOX.lists.".to_string().into()),
                    Value::Variable(VariableType::Match(2)),
                ]),
            ),
            (
                "Ein unerh${unicode:00F6}rt gro${unicode:00DF}er Test",
                Value::Text("Ein unerhört großer Test".to_string().into()),
            ),
            ("&%${}!", Value::Text("&%${}!".to_string().into())),
            ("${doh!}", Value::Text("${doh!}".to_string().into())),
            (
                "${hex: 20 }${global.hi}${hex: 20 }",
                Value::List(vec![
                    Value::Text(" ".to_string().into()),
                    Value::Variable(VariableType::Global("hi".to_string())),
                    Value::Text(" ".to_string().into()),
                ]),
            ),
            (
                "${hex:20 24 7b z}${global.hi}${unicode:}${unicode: }${hex:20}",
                Value::List(vec![
                    Value::Text("${hex:20 24 7b z}".to_string().into()),
                    Value::Variable(VariableType::Global("hi".to_string())),
                    Value::Text("${unicode:}${unicode: } ".to_string().into()),
                ]),
            ),
            (
                "${header.from}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Text,
                    index_hdr: -1,
                    index_part: -1,
                })),
            ),
            (
                "${header.from.addr}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Address(AddressPart::All),
                    index_hdr: -1,
                    index_part: -1,
                })),
            ),
            (
                "${header.from[1]}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Text,
                    index_hdr: 1,
                    index_part: -1,
                })),
            ),
            (
                "${header.from[*]}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Text,
                    index_hdr: 0,
                    index_part: -1,
                })),
            ),
            (
                "${header.from[20].name}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Address(AddressPart::Name),
                    index_hdr: 20,
                    index_part: -1,
                })),
            ),
            (
                "${header.from[*].addr}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Address(AddressPart::All),
                    index_hdr: 0,
                    index_part: -1,
                })),
            ),
            (
                "${header.from[-5].name[2]}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Address(AddressPart::Name),
                    index_hdr: -5,
                    index_part: 2,
                })),
            ),
            (
                "${header.from[*].raw[*]}",
                Value::Variable(VariableType::Header(HeaderVariable {
                    name: vec![HeaderName::From],
                    part: HeaderPart::Raw,
                    index_hdr: 0,
                    index_part: 0,
                })),
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
