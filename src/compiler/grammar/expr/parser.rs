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

use crate::compiler::VariableType;

use super::{tokenizer::Tokenizer, BinaryOperator, Expression, Token};

pub struct ExpressionParser<'x, F>
where
    F: Fn(&str, bool) -> Result<VariableType, String>,
{
    pub(crate) tokenizer: Tokenizer<'x, F>,
    pub(crate) output: Vec<Expression>,
    operator_stack: Vec<Token>,
}

impl<'x, F> ExpressionParser<'x, F>
where
    F: Fn(&str, bool) -> Result<VariableType, String>,
{
    pub fn from_tokenizer(tokenizer: Tokenizer<'x, F>) -> Self {
        Self {
            tokenizer,
            output: Vec::new(),
            operator_stack: Vec::new(),
        }
    }

    pub fn parse(mut self) -> Result<Self, String> {
        while let Some(token) = self.tokenizer.next()? {
            match token {
                Token::Variable(v) => self.output.push(Expression::Variable(v)),
                Token::Number(n) => self.output.push(Expression::Number(n)),
                Token::UnaryOperator(uop) => self.operator_stack.push(Token::UnaryOperator(uop)),
                Token::OpenParen => self.operator_stack.push(token),
                Token::CloseParen => {
                    while let Some(token) = self.operator_stack.pop() {
                        match token {
                            Token::OpenParen => break,
                            Token::BinaryOperator(bop) => {
                                self.output.push(Expression::BinaryOperator(bop))
                            }
                            Token::UnaryOperator(uop) => {
                                self.output.push(Expression::UnaryOperator(uop))
                            }
                            _ => return Err("Mismatched parentheses".to_string()),
                        }
                    }
                }
                Token::BinaryOperator(bop) => {
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
