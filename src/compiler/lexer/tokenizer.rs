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

use std::{iter::Peekable, slice::Iter};

use crate::{
    compiler::{CompileError, ErrorType, Number},
    runtime::eval::IntoString,
    Compiler,
};

use super::{word::WORDS, StringConstant, Token};

pub(crate) struct Tokenizer<'x> {
    pub compiler: &'x Compiler,
    pub iter: Peekable<Iter<'x, u8>>,
    pub buf: Vec<u8>,
    pub next_token: Vec<TokenInfo>,

    pub pos: usize,
    pub line_num: usize,
    pub line_start: usize,

    pub text_line_num: usize,
    pub text_line_pos: usize,

    pub token_line_num: usize,
    pub token_line_pos: usize,

    pub token_is_tag: bool,

    pub last_ch: u8,
    pub state: State,
}

#[derive(Debug)]
pub(crate) struct TokenInfo {
    pub(crate) token: Token,
    pub(crate) line_num: usize,
    pub(crate) line_pos: usize,
}

pub(crate) enum State {
    None,
    BracketComment,
    HashComment,
    QuotedString(StringType),
    MultiLine(StringType),
}

#[derive(Clone, Copy, Default)]
pub(crate) struct StringType {
    maybe_variable: bool,
    has_other: bool,
    has_digits: bool,
    has_dots: bool,
}

impl<'x> Tokenizer<'x> {
    pub fn new(compiler: &'x Compiler, bytes: &'x [u8]) -> Self {
        Tokenizer {
            compiler,
            iter: bytes.iter().peekable(),
            buf: Vec::with_capacity(bytes.len() / 2),
            pos: usize::MAX,
            line_num: 1,
            line_start: 0,
            text_line_num: 0,
            text_line_pos: 0,
            token_line_num: 0,
            token_line_pos: 0,
            token_is_tag: false,
            next_token: Vec::with_capacity(2),
            last_ch: 0,
            state: State::None,
        }
    }

    pub fn get_current_token(&mut self) -> Option<TokenInfo> {
        if !self.buf.is_empty() {
            let word = std::str::from_utf8(&self.buf).unwrap();
            let token = if let Some(word) = WORDS.get(word) {
                if self.token_is_tag {
                    self.token_line_pos -= 1;
                    Token::Tag(*word)
                } else {
                    Token::Identifier(*word)
                }
            } else if self.buf.first().unwrap().is_ascii_digit() {
                let multiplier = match self.buf.last().unwrap() {
                    b'k' => 1024,
                    b'm' => 1048576,
                    b'g' => 1073741824,
                    _ => 1,
                };

                if let Ok(number) = (if multiplier > 1 && self.buf.len() > 1 {
                    std::str::from_utf8(&self.buf[..self.buf.len() - 1]).unwrap()
                } else {
                    word
                })
                .parse::<usize>()
                {
                    Token::Number(number.saturating_mul(multiplier))
                } else if self.token_is_tag {
                    Token::Unknown(format!(":{word}"))
                } else {
                    Token::Unknown(word.to_string())
                }
            } else if self.token_is_tag {
                Token::Unknown(format!(":{word}"))
            } else {
                Token::Unknown(word.to_string())
            };

            self.reset_current_token();

            Some(TokenInfo {
                token,
                line_num: self.token_line_num,
                line_pos: self.token_line_pos,
            })
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn reset_current_token(&mut self) {
        self.buf.clear();
        self.token_is_tag = false;
    }

    #[inline(always)]
    pub fn token_is_tag(&mut self) {
        self.token_is_tag = true;
    }

    pub fn get_token(&mut self, token: Token) -> TokenInfo {
        let next_token = TokenInfo {
            token,
            line_num: self.line_num,
            line_pos: self.pos - self.line_start,
        };
        if let Some(token) = self.get_current_token() {
            self.next_token.push(next_token);
            token
        } else {
            next_token
        }
    }

    pub fn get_string(&mut self, str_type: StringType) -> Result<TokenInfo, CompileError> {
        if self.buf.len() < self.compiler.max_string_size {
            let token = if str_type.maybe_variable {
                Token::StringVariable(self.buf.to_vec())
            } else {
                let constant = self.buf.to_vec().into_string();
                if !str_type.has_other && str_type.has_digits {
                    if !str_type.has_dots {
                        if let Some(number) = constant.parse::<i64>().ok().and_then(|n| {
                            if n.to_string() == constant {
                                Some(n)
                            } else {
                                None
                            }
                        }) {
                            Token::StringConstant(StringConstant::Number(Number::Integer(number)))
                        } else {
                            Token::StringConstant(StringConstant::String(constant))
                        }
                    } else if let Some(number) = constant.parse::<f64>().ok().and_then(|n| {
                        if n.to_string() == constant {
                            Some(n)
                        } else {
                            None
                        }
                    }) {
                        Token::StringConstant(StringConstant::Number(Number::Float(number)))
                    } else {
                        Token::StringConstant(StringConstant::String(constant))
                    }
                } else {
                    Token::StringConstant(StringConstant::String(constant))
                }
            };

            self.buf.clear();

            Ok(TokenInfo {
                token,
                line_num: self.text_line_num,
                line_pos: self.text_line_pos,
            })
        } else {
            Err(CompileError {
                line_num: self.text_line_num,
                line_pos: self.text_line_pos,
                error_type: ErrorType::StringTooLong,
            })
        }
    }

    #[inline(always)]
    pub fn push_byte(&mut self, ch: u8) {
        if self.buf.is_empty() {
            self.token_line_num = self.line_num;
            self.token_line_pos = self.pos - self.line_start;
        }
        self.buf.push(ch);
    }

    #[inline(always)]
    pub fn new_line(&mut self) {
        self.line_num += 1;
        self.line_start = self.pos;
    }

    #[inline(always)]
    pub fn text_start(&mut self) {
        self.text_line_num = self.line_num;
        self.text_line_pos = self.pos - self.line_start;
    }

    #[inline(always)]
    pub fn is_token_start(&self) -> bool {
        self.buf.is_empty()
    }

    #[inline(always)]
    pub fn token_bytes(&self) -> &[u8] {
        &self.buf
    }

    #[inline(always)]
    pub fn next_byte(&mut self) -> Option<(u8, u8)> {
        self.iter.next().map(|&ch| {
            let last_ch = self.last_ch;
            self.pos = self.pos.wrapping_add(1);
            self.last_ch = ch;
            (ch, last_ch)
        })
    }

    #[inline(always)]
    pub fn peek_byte(&mut self) -> Option<u8> {
        self.iter.peek().map(|ch| **ch)
    }

    pub fn unwrap_next(&mut self) -> Result<TokenInfo, CompileError> {
        if let Some(token) = self.next() {
            token
        } else {
            Err(CompileError {
                line_num: self.line_num,
                line_pos: self.pos - self.line_start,
                error_type: ErrorType::UnexpectedEOF,
            })
        }
    }

    pub fn expect_token(&mut self, token: Token) -> Result<(), CompileError> {
        let next_token = self.unwrap_next()?;
        if next_token.token == token {
            Ok(())
        } else {
            Err(next_token.expected(format!("'{token}'")))
        }
    }

    pub fn expect_static_string(&mut self) -> Result<String, CompileError> {
        let next_token = self.unwrap_next()?;
        match next_token.token {
            Token::StringConstant(s) => Ok(s.into_string()),
            Token::BracketOpen => {
                let mut string = None;
                loop {
                    let token_info = self.unwrap_next()?;
                    match token_info.token {
                        Token::StringConstant(string_) => {
                            string = string_.into();
                        }
                        Token::BracketClose if string.is_some() => break,
                        _ => return Err(token_info.expected("constant string")),
                    }
                }
                Ok(string.unwrap().into_string())
            }
            _ => Err(next_token.expected("constant string")),
        }
    }

    pub fn expect_number(&mut self, max_value: usize) -> Result<usize, CompileError> {
        let next_token = self.unwrap_next()?;
        if let Token::Number(n) = next_token.token {
            if n < max_value {
                Ok(n)
            } else {
                Err(next_token.expected(format!("number lower than {max_value}")))
            }
        } else {
            Err(next_token.expected("number"))
        }
    }

    pub fn invalid_character(&self) -> CompileError {
        CompileError {
            line_num: self.line_num,
            line_pos: self.pos - self.line_start,
            error_type: ErrorType::InvalidCharacter(self.last_ch),
        }
    }

    pub fn peek(&mut self) -> Option<Result<&TokenInfo, CompileError>> {
        if self.next_token.is_empty() {
            match self.next()? {
                Ok(next_token) => self.next_token.push(next_token),
                Err(err) => return Some(Err(err)),
            }
        }
        self.next_token.last().map(Ok)
    }
}

impl<'x> Iterator for Tokenizer<'x> {
    type Item = Result<TokenInfo, CompileError>;

    fn next(&mut self) -> Option<Self::Item> {
        if let Some(prev_token) = self.next_token.pop() {
            return Some(Ok(prev_token));
        }

        'outer: while let Some((ch, last_ch)) = self.next_byte() {
            match self.state {
                State::None => match ch {
                    b'a'..=b'z' | b'0'..=b'9' | b'_' | b'.' | b'$' => {
                        self.push_byte(ch);
                    }
                    b'A'..=b'Z' => {
                        self.push_byte(ch.to_ascii_lowercase());
                    }
                    b':' => {
                        if self.is_token_start() {
                            self.token_is_tag();
                        } else if self.token_bytes().eq_ignore_ascii_case(b"text") {
                            self.state = State::MultiLine(StringType::default());
                            self.text_start();
                            while let Some((ch, _)) = self.next_byte() {
                                if ch == b'\n' {
                                    self.new_line();
                                    self.reset_current_token();
                                    continue 'outer;
                                }
                            }
                        } else {
                            return Some(Err(self.invalid_character()));
                        }
                    }
                    b'"' => {
                        self.state = State::QuotedString(StringType::default());
                        self.text_start();
                        if let Some(token) = self.get_current_token() {
                            return Some(Ok(token));
                        }
                    }
                    b'{' => {
                        return Some(Ok(self.get_token(Token::CurlyOpen)));
                    }
                    b'}' => {
                        return Some(Ok(self.get_token(Token::CurlyClose)));
                    }
                    b';' => {
                        return Some(Ok(self.get_token(Token::Semicolon)));
                    }
                    b',' => {
                        return Some(Ok(self.get_token(Token::Comma)));
                    }
                    b'[' => {
                        return Some(Ok(self.get_token(Token::BracketOpen)));
                    }
                    b']' => {
                        return Some(Ok(self.get_token(Token::BracketClose)));
                    }
                    b'(' => {
                        return Some(Ok(self.get_token(Token::ParenthesisOpen)));
                    }
                    b')' => {
                        return Some(Ok(self.get_token(Token::ParenthesisClose)));
                    }
                    b'/' => {
                        if let Some((b'*', _)) = self.next_byte() {
                            self.last_ch = 0;
                            self.state = State::BracketComment;
                            self.text_start();
                            if let Some(token) = self.get_current_token() {
                                return Some(Ok(token));
                            }
                        } else {
                            return Some(Err(self.invalid_character()));
                        }
                    }
                    b'#' => {
                        self.state = State::HashComment;
                        if let Some(token) = self.get_current_token() {
                            return Some(Ok(token));
                        }
                    }
                    b'\n' => {
                        self.new_line();
                        if let Some(token) = self.get_current_token() {
                            return Some(Ok(token));
                        }
                    }
                    b' ' | b'\t' | b'\r' => {
                        if let Some(token) = self.get_current_token() {
                            return Some(Ok(token));
                        }
                    }
                    _ => {
                        return Some(Err(self.invalid_character()));
                    }
                },
                State::BracketComment { .. } => match ch {
                    b'/' if last_ch == b'*' => {
                        self.state = State::None;
                    }
                    b'\n' => {
                        self.new_line();
                    }
                    _ => (),
                },
                State::HashComment => {
                    if ch == b'\n' {
                        self.state = State::None;
                        self.new_line();
                    }
                }
                State::QuotedString(mut str_type) => match ch {
                    b'"' if last_ch != b'\\' => {
                        self.state = State::None;
                        return Some(self.get_string(str_type));
                    }
                    b'\n' => {
                        self.new_line();
                        self.push_byte(b'\n');
                        str_type.has_other = true;
                        self.state = State::QuotedString(str_type);
                    }
                    b'{' if (last_ch == b'$' || last_ch == b'%') => {
                        str_type.maybe_variable = true;
                        self.state = State::QuotedString(str_type);
                        self.push_byte(ch);
                    }
                    b'\\' => {
                        if last_ch == b'\\' {
                            self.push_byte(ch);
                        }
                    }
                    b'0'..=b'9' => {
                        if !str_type.has_digits {
                            str_type.has_digits = true;
                            self.state = State::QuotedString(str_type);
                        }
                        self.push_byte(ch);
                    }
                    b'.' => {
                        if !str_type.has_dots {
                            str_type.has_dots = true;
                        } else {
                            str_type.has_other = true;
                        }
                        self.state = State::QuotedString(str_type);
                        self.push_byte(ch);
                    }
                    _ => {
                        if !str_type.has_other && ch != b'-' {
                            str_type.has_other = true;
                            self.state = State::QuotedString(str_type);
                        }
                        self.push_byte(ch);
                    }
                },
                State::MultiLine(mut str_type) => match ch {
                    b'.' if last_ch == b'\n' => {
                        let is_eof = match (self.next_byte(), self.peek_byte()) {
                            (Some((b'\r', _)), Some(b'\n')) => {
                                self.next_byte();
                                true
                            }
                            (Some((b'\n', _)), _) => true,
                            (Some((b'.', _)), _) => {
                                self.push_byte(b'.');
                                false
                            }
                            (Some((ch, _)), _) => {
                                self.push_byte(b'.');
                                self.push_byte(ch);
                                false
                            }
                            _ => false,
                        };

                        if is_eof {
                            self.new_line();
                            self.state = State::None;
                            return Some(self.get_string(str_type));
                        }
                    }
                    b'\n' => {
                        self.new_line();
                        self.push_byte(b'\n');
                    }
                    b'{' if (last_ch == b'$' || last_ch == b'%') => {
                        str_type.maybe_variable = true;
                        self.state = State::MultiLine(str_type);
                        self.push_byte(ch);
                    }
                    b'0'..=b'9' => {
                        if !str_type.has_digits {
                            str_type.has_digits = true;
                            self.state = State::MultiLine(str_type);
                        }
                        self.push_byte(ch);
                    }
                    b'.' => {
                        if !str_type.has_dots {
                            str_type.has_dots = true;
                        } else {
                            str_type.has_other = true;
                        }
                        self.state = State::MultiLine(str_type);
                        self.push_byte(ch);
                    }
                    _ => {
                        if !str_type.has_other && ch != b'-' {
                            str_type.has_other = true;
                            self.state = State::MultiLine(str_type);
                        }
                        self.push_byte(ch);
                    }
                },
            }
        }

        match self.state {
            State::BracketComment | State::QuotedString(_) | State::MultiLine(_) => {
                Some(Err(CompileError {
                    line_num: self.text_line_num,
                    line_pos: self.text_line_pos,
                    error_type: (&self.state).into(),
                }))
            }
            _ => None,
        }
    }
}

impl From<&State> for ErrorType {
    fn from(state: &State) -> Self {
        match state {
            State::BracketComment => ErrorType::UnterminatedComment,
            State::QuotedString(_) => ErrorType::UnterminatedString,
            State::MultiLine(_) => ErrorType::UnterminatedMultiline,
            _ => unreachable!(),
        }
    }
}
