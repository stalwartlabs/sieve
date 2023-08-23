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

use crate::{
    compiler::{Number, VariableType},
    runtime::eval::IntoString,
};

use super::{BinaryOperator, Token, UnaryOperator};

pub struct Tokenizer<'x, F>
where
    F: Fn(&str, bool) -> Result<VariableType, String>,
{
    pub(crate) iter: Peekable<Enumerate<Iter<'x, u8>>>,
    variable_map: F,
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
    F: Fn(&str, bool) -> Result<VariableType, String>,
{
    pub fn new(expr: &'x str, variable_map: F) -> Self {
        Self::from_iter(expr.as_bytes().iter().enumerate().peekable(), variable_map)
    }

    #[allow(clippy::should_implement_trait)]
    pub(crate) fn from_iter(iter: Peekable<Enumerate<Iter<'x, u8>>>, variable_map: F) -> Self {
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
            variable_map,
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
                        b' ' => {
                            if prev_token.is_some() {
                                return Ok(prev_token);
                            } else {
                                continue;
                            }
                        }
                        _ => {
                            return Err(format!("Invalid character {ch}",));
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
            let result = (self.variable_map)(&buf, self.has_dot).map(Token::Variable);
            self.has_alpha = false;
            self.has_number = false;
            self.has_dot = false;
            result
        }
    }
}
