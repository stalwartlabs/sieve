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

use std::{
    iter::{Enumerate, Peekable},
    slice::Iter,
};

use crate::{compiler::Number, runtime::eval::IntoString};

use super::{BinaryOperator, Token, UnaryOperator};

pub(crate) struct Tokenizer<'x, F>
where
    F: Fn(&str, bool) -> Result<Token, String>,
{
    pub(crate) iter: Peekable<Enumerate<Iter<'x, u8>>>,
    token_map: F,
    buf: Vec<u8>,
    depth: u32,
    next_token: Vec<Token>,
    has_number: bool,
    has_dot: bool,
    has_alpha: bool,
    is_start: bool,
    is_eof: bool,
}

impl<'x, F> Tokenizer<'x, F>
where
    F: Fn(&str, bool) -> Result<Token, String>,
{
    #[cfg(test)]
    pub fn new(expr: &'x str, token_map: F) -> Self {
        Self::from_iter(expr.as_bytes().iter().enumerate().peekable(), token_map)
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn from_iter(iter: Peekable<Enumerate<Iter<'x, u8>>>, token_map: F) -> Self {
        Self {
            iter,
            buf: Vec::new(),
            depth: 0,
            next_token: Vec::with_capacity(2),
            has_number: false,
            has_dot: false,
            has_alpha: false,
            is_start: true,
            is_eof: false,
            token_map,
        }
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn next(&mut self) -> Result<Option<Token>, String> {
        if let Some(token) = self.next_token.pop() {
            return Ok(Some(token));
        } else if self.is_eof {
            return Ok(None);
        }

        while let Some((_, &ch)) = self.iter.next() {
            match ch {
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => {
                    self.buf.push(ch);
                    self.has_alpha = true;
                }
                b'0'..=b'9' => {
                    self.buf.push(ch);
                    self.has_number = true;
                }
                b'.' => {
                    self.buf.push(ch);
                    self.has_dot = true;
                }
                b'}' => {
                    self.is_eof = true;
                    break;
                }
                b'[' if matches!(self.buf.get(0..7), Some(b"header.")) => {
                    self.buf.push(ch);
                }
                b'-' if self.buf.last().map_or(false, |c| *c == b'[')
                    || matches!(self.buf.get(0..7), Some(b"header.")) =>
                {
                    self.buf.push(ch);
                }
                b':' if self.buf.contains(&b'.') => {
                    self.buf.push(ch);
                }
                b']' if self.buf.contains(&b'[') => {
                    self.buf.push(b']');
                }
                b'*' if self.buf.last().map_or(false, |&c| c == b'[' || c == b'.') => {
                    self.buf.push(ch);
                }
                _ => {
                    let prev_token = if !self.buf.is_empty() {
                        self.is_start = false;
                        self.parse_buf()?.into()
                    } else {
                        None
                    };
                    let token = match ch {
                        b'&' => {
                            if matches!(self.iter.peek(), Some((_, b'&'))) {
                                self.iter.next();
                            }
                            Token::BinaryOperator(BinaryOperator::And)
                        }
                        b'|' => {
                            if matches!(self.iter.peek(), Some((_, b'|'))) {
                                self.iter.next();
                            }
                            Token::BinaryOperator(BinaryOperator::Or)
                        }
                        b'!' => {
                            if matches!(self.iter.peek(), Some((_, b'='))) {
                                self.iter.next();
                                Token::BinaryOperator(BinaryOperator::Ne)
                            } else {
                                Token::UnaryOperator(UnaryOperator::Not)
                            }
                        }
                        b'^' => Token::BinaryOperator(BinaryOperator::Xor),
                        b'(' => {
                            self.depth += 1;
                            Token::OpenParen
                        }
                        b')' => {
                            if self.depth == 0 {
                                return Err("Unmatched close parenthesis".to_string());
                            }
                            self.depth -= 1;
                            Token::CloseParen
                        }
                        b'+' => Token::BinaryOperator(BinaryOperator::Add),
                        b'*' => Token::BinaryOperator(BinaryOperator::Multiply),
                        b'/' => Token::BinaryOperator(BinaryOperator::Divide),
                        b'-' => {
                            if self.is_start {
                                Token::UnaryOperator(UnaryOperator::Minus)
                            } else {
                                Token::BinaryOperator(BinaryOperator::Subtract)
                            }
                        }
                        b'=' => match self.iter.next() {
                            Some((_, b'=')) => Token::BinaryOperator(BinaryOperator::Eq),
                            Some((_, b'>')) => Token::BinaryOperator(BinaryOperator::Ge),
                            Some((_, b'<')) => Token::BinaryOperator(BinaryOperator::Le),
                            _ => Token::BinaryOperator(BinaryOperator::Eq),
                        },
                        b'>' => match self.iter.peek() {
                            Some((_, b'=')) => {
                                self.iter.next();
                                Token::BinaryOperator(BinaryOperator::Ge)
                            }
                            _ => Token::BinaryOperator(BinaryOperator::Gt),
                        },
                        b'<' => match self.iter.peek() {
                            Some((_, b'=')) => {
                                self.iter.next();
                                Token::BinaryOperator(BinaryOperator::Le)
                            }
                            _ => Token::BinaryOperator(BinaryOperator::Lt),
                        },
                        b',' => Token::Comma,
                        b'[' => Token::OpenBracket,
                        b']' => Token::CloseBracket,
                        b' ' | b'\r' | b'\n' => {
                            if prev_token.is_some() {
                                return Ok(prev_token);
                            } else {
                                continue;
                            }
                        }
                        b'\"' | b'\'' => {
                            let mut buf = Vec::with_capacity(16);
                            let stop_ch = ch;
                            let mut last_ch = 0;
                            let mut found_end = false;

                            for (_, &ch) in self.iter.by_ref() {
                                if ch == stop_ch && last_ch != b'\\' {
                                    found_end = true;
                                    break;
                                } else if ch != b'\\' || last_ch == b'\\' {
                                    buf.push(ch);
                                }
                                last_ch = ch;
                            }

                            if found_end {
                                Token::String(
                                    String::from_utf8(buf)
                                        .map_err(|_| "Invalid UTF-8".to_string())?,
                                )
                            } else {
                                return Err("Unterminated string".to_string());
                            }
                        }
                        _ => {
                            return Err(format!("Invalid character {:?}", char::from(ch),));
                        }
                    };
                    self.is_start = ch == b'(';

                    return if prev_token.is_some() {
                        self.next_token.push(token);
                        Ok(prev_token)
                    } else {
                        Ok(Some(token))
                    };
                }
            }
        }

        if self.depth > 0 {
            Err("Unmatched open parenthesis".to_string())
        } else if !self.buf.is_empty() {
            self.parse_buf().map(Some)
        } else {
            Ok(None)
        }
    }

    fn parse_buf(&mut self) -> Result<Token, String> {
        let buf = std::mem::take(&mut self.buf).into_string();
        if self.has_number && !self.has_alpha {
            self.has_number = false;
            if self.has_dot {
                self.has_dot = false;

                buf.parse::<f64>()
                    .map(|f| Token::Number(Number::Float(f)))
                    .map_err(|_| format!("Invalid float value {}", buf,))
            } else {
                buf.parse::<i64>()
                    .map(|i| Token::Number(Number::Integer(i)))
                    .map_err(|_| format!("Invalid integer value {}", buf,))
            }
        } else {
            let has_dot = self.has_dot;
            let has_number = self.has_number;

            self.has_alpha = false;
            self.has_number = false;
            self.has_dot = false;

            if !has_number && !has_dot && [4, 5].contains(&buf.len()) {
                if buf == "true" {
                    return Ok(Token::Number(Number::Integer(1)));
                } else if buf == "false" {
                    return Ok(Token::Number(Number::Integer(0)));
                }
            }

            (self.token_map)(&buf, has_dot)
        }
    }
}
