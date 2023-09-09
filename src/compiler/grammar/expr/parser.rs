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

use super::{tokenizer::Tokenizer, BinaryOperator, Expression, Token};

pub(crate) struct ExpressionParser<'x, F>
where
    F: Fn(&str, bool) -> Result<Token, String>,
{
    pub(crate) tokenizer: Tokenizer<'x, F>,
    pub(crate) output: Vec<Expression>,
    operator_stack: Vec<Token>,
}

impl<'x, F> ExpressionParser<'x, F>
where
    F: Fn(&str, bool) -> Result<Token, String>,
{
    pub fn from_tokenizer(tokenizer: Tokenizer<'x, F>) -> Self {
        Self {
            tokenizer,
            output: Vec::new(),
            operator_stack: Vec::new(),
        }
    }

    pub fn parse(mut self) -> Result<Self, String> {
        let mut arg_count: Vec<i32> = vec![];

        while let Some(token) = self.tokenizer.next()? {
            match token {
                Token::Variable(v) => {
                    if let Some(x) = arg_count.last_mut() {
                        *x = x.saturating_add(1);
                    }
                    self.output.push(Expression::Variable(v))
                }
                Token::Number(n) => {
                    if let Some(x) = arg_count.last_mut() {
                        *x = x.saturating_add(1);
                    }
                    self.output.push(Expression::Number(n))
                }
                Token::String(s) => {
                    if let Some(x) = arg_count.last_mut() {
                        *x = x.saturating_add(1);
                    }
                    self.output.push(Expression::String(s))
                }
                Token::UnaryOperator(uop) => self.operator_stack.push(Token::UnaryOperator(uop)),
                Token::OpenParen => self.operator_stack.push(token),
                Token::CloseParen => {
                    loop {
                        match self.operator_stack.pop() {
                            Some(Token::OpenParen) => {
                                break;
                            }
                            Some(Token::BinaryOperator(bop)) => {
                                self.output.push(Expression::BinaryOperator(bop))
                            }
                            Some(Token::UnaryOperator(uop)) => {
                                self.output.push(Expression::UnaryOperator(uop))
                            }
                            _ => return Err("Mismatched parentheses".to_string()),
                        }
                    }

                    if let Some(Token::Function { id, num_args, name }) = self.operator_stack.last()
                    {
                        let got_args = arg_count.pop().unwrap();
                        if got_args != *num_args as i32 {
                            return Err(format!(
                                "Expression function {:?} expected {} arguments, got {}",
                                name, num_args, got_args
                            ));
                        }
                        let expr = Expression::Function {
                            id: *id,
                            num_args: *num_args,
                        };
                        self.operator_stack.pop();
                        self.output.push(expr);
                    }
                }
                Token::BinaryOperator(bop) => {
                    if let Some(x) = arg_count.last_mut() {
                        *x = x.saturating_sub(1);
                    }
                    while let Some(top_token) = self.operator_stack.last() {
                        match top_token {
                            Token::BinaryOperator(top_bop) => {
                                if bop.precedence() <= top_bop.precedence() {
                                    let top_bop = *top_bop;
                                    self.operator_stack.pop();
                                    self.output.push(Expression::BinaryOperator(top_bop));
                                } else {
                                    break;
                                }
                            }
                            Token::UnaryOperator(top_uop) => {
                                let top_uop = *top_uop;
                                self.operator_stack.pop();
                                self.output.push(Expression::UnaryOperator(top_uop));
                            }
                            _ => break,
                        }
                    }
                    self.operator_stack.push(Token::BinaryOperator(bop));
                }
                Token::Function { id, name, num_args } => {
                    if let Some(x) = arg_count.last_mut() {
                        *x = x.saturating_add(1);
                    }
                    arg_count.push(0);
                    self.operator_stack
                        .push(Token::Function { id, name, num_args })
                }
                Token::Comma => {
                    while let Some(token) = self.operator_stack.last() {
                        match token {
                            Token::OpenParen => break,
                            Token::BinaryOperator(bop) => {
                                self.output.push(Expression::BinaryOperator(*bop));
                                self.operator_stack.pop();
                            }
                            Token::UnaryOperator(uop) => {
                                self.output.push(Expression::UnaryOperator(*uop));
                                self.operator_stack.pop();
                            }
                            _ => break,
                        }
                    }
                }
            }
        }

        while let Some(token) = self.operator_stack.pop() {
            match token {
                Token::BinaryOperator(bop) => self.output.push(Expression::BinaryOperator(bop)),
                Token::UnaryOperator(uop) => self.output.push(Expression::UnaryOperator(uop)),
                _ => return Err("Invalid token on the operator stack".to_string()),
            }
        }

        Ok(self)
    }
}

impl BinaryOperator {
    fn precedence(&self) -> i32 {
        match self {
            BinaryOperator::Multiply | BinaryOperator::Divide => 7,
            BinaryOperator::Add | BinaryOperator::Subtract => 6,
            BinaryOperator::Gt | BinaryOperator::Ge | BinaryOperator::Lt | BinaryOperator::Le => 5,
            BinaryOperator::Eq | BinaryOperator::Ne => 4,
            BinaryOperator::Xor => 3,
            BinaryOperator::And => 2,
            BinaryOperator::Or => 1,
        }
    }
}
