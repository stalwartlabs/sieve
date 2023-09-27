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
    operator_stack: Vec<(Token, Option<usize>)>,
    arg_count: Vec<i32>,
}

pub(crate) const ID_ARRAY_ACCESS: u32 = u32::MAX;
pub(crate) const ID_ARRAY_BUILD: u32 = u32::MAX - 1;
pub(crate) const ID_EXTERNAL: u32 = u32::MAX - 2;

impl<'x, F> ExpressionParser<'x, F>
where
    F: Fn(&str, bool) -> Result<Token, String>,
{
    pub fn from_tokenizer(tokenizer: Tokenizer<'x, F>) -> Self {
        Self {
            tokenizer,
            output: Vec::new(),
            operator_stack: Vec::new(),
            arg_count: Vec::new(),
        }
    }

    pub fn parse(mut self) -> Result<Self, String> {
        let mut last_is_var_or_fnc = false;

        while let Some(token) = self.tokenizer.next()? {
            let mut is_var_or_fnc = false;
            match token {
                Token::Variable(v) => {
                    self.inc_arg_count();
                    is_var_or_fnc = true;
                    self.output.push(Expression::Variable(v))
                }
                Token::Number(n) => {
                    self.inc_arg_count();
                    self.output.push(Expression::Constant(n.into()))
                }
                Token::String(s) => {
                    self.inc_arg_count();
                    self.output.push(Expression::Constant(s.into()))
                }
                Token::UnaryOperator(uop) => {
                    self.operator_stack.push((Token::UnaryOperator(uop), None))
                }
                Token::OpenParen => self.operator_stack.push((token, None)),
                Token::CloseParen | Token::CloseBracket => {
                    let expect_token = if matches!(token, Token::CloseParen) {
                        Token::OpenParen
                    } else {
                        Token::OpenBracket
                    };
                    loop {
                        match self.operator_stack.pop() {
                            Some((t, _)) if t == expect_token => {
                                break;
                            }
                            Some((Token::BinaryOperator(bop), jmp_pos)) => {
                                self.update_jmp_pos(jmp_pos);
                                self.output.push(Expression::BinaryOperator(bop))
                            }
                            Some((Token::UnaryOperator(uop), _)) => {
                                self.output.push(Expression::UnaryOperator(uop))
                            }
                            _ => return Err("Mismatched parentheses".to_string()),
                        }
                    }

                    if let Some((Token::Function { id, num_args, name }, _)) =
                        self.operator_stack.last()
                    {
                        let got_args = self.arg_count.pop().unwrap();
                        if got_args != *num_args as i32 {
                            return Err(if *id != u32::MAX {
                                format!(
                                    "Expression function {:?} expected {} arguments, got {}",
                                    name, num_args, got_args
                                )
                            } else {
                                "Missing array index".to_string()
                            });
                        }

                        let expr = match *id {
                            ID_ARRAY_ACCESS => Expression::ArrayAccess,
                            ID_ARRAY_BUILD => Expression::ArrayBuild(*num_args),
                            id => Expression::Function {
                                id,
                                num_args: *num_args,
                            },
                        };

                        self.operator_stack.pop();
                        self.output.push(expr);
                    }

                    is_var_or_fnc = true;
                }
                Token::BinaryOperator(bop) => {
                    self.dec_arg_count();
                    while let Some((top_token, prev_jmp_pos)) = self.operator_stack.last() {
                        match top_token {
                            Token::BinaryOperator(top_bop) => {
                                if bop.precedence() <= top_bop.precedence() {
                                    let top_bop = *top_bop;
                                    let jmp_pos = *prev_jmp_pos;
                                    self.update_jmp_pos(jmp_pos);
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

                    // Add jump instruction for short-circuiting
                    let jmp_pos = match bop {
                        BinaryOperator::And => {
                            self.output.push(Expression::JmpIf { val: false, pos: 0 });
                            Some(self.output.len() - 1)
                        }
                        BinaryOperator::Or => {
                            self.output.push(Expression::JmpIf { val: true, pos: 0 });
                            Some(self.output.len() - 1)
                        }
                        _ => None,
                    };

                    self.operator_stack
                        .push((Token::BinaryOperator(bop), jmp_pos));
                }
                Token::Function { id, name, num_args } => {
                    self.inc_arg_count();
                    self.arg_count.push(0);
                    self.operator_stack
                        .push((Token::Function { id, name, num_args }, None))
                }
                Token::OpenBracket => {
                    // Array functions
                    let (id, num_args, arg_count) = if last_is_var_or_fnc {
                        (ID_ARRAY_ACCESS, 2, 1)
                    } else {
                        self.inc_arg_count();
                        (ID_ARRAY_BUILD, 0, 0)
                    };
                    self.arg_count.push(arg_count);
                    self.operator_stack.push((
                        Token::Function {
                            id,
                            name: String::from("array"),
                            num_args,
                        },
                        None,
                    ));
                    self.operator_stack.push((token, None));
                }
                Token::Comma => {
                    while let Some((token, jmp_pos)) = self.operator_stack.last() {
                        match token {
                            Token::OpenParen => break,
                            Token::BinaryOperator(bop) => {
                                let bop = *bop;
                                let jmp_pos = *jmp_pos;
                                self.update_jmp_pos(jmp_pos);
                                self.output.push(Expression::BinaryOperator(bop));
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
            last_is_var_or_fnc = is_var_or_fnc;
        }

        while let Some((token, jmp_pos)) = self.operator_stack.pop() {
            match token {
                Token::BinaryOperator(bop) => {
                    self.update_jmp_pos(jmp_pos);
                    self.output.push(Expression::BinaryOperator(bop))
                }
                Token::UnaryOperator(uop) => self.output.push(Expression::UnaryOperator(uop)),
                _ => return Err("Invalid token on the operator stack".to_string()),
            }
        }

        Ok(self)
    }

    fn inc_arg_count(&mut self) {
        if let Some(x) = self.arg_count.last_mut() {
            *x = x.saturating_add(1);
            let op_pos = self.operator_stack.len() - 2;
            match self.operator_stack.get_mut(op_pos) {
                Some((Token::Function { num_args, id, .. }, _)) if *id == ID_ARRAY_BUILD => {
                    *num_args += 1;
                }
                _ => {}
            }
        }
    }

    fn dec_arg_count(&mut self) {
        if let Some(x) = self.arg_count.last_mut() {
            *x = x.saturating_sub(1);
        }
    }

    fn update_jmp_pos(&mut self, jmp_pos: Option<usize>) {
        if let Some(jmp_pos) = jmp_pos {
            let cur_pos = self.output.len();
            if let Expression::JmpIf { pos, .. } = &mut self.output[jmp_pos] {
                *pos = (cur_pos - jmp_pos) as u32;
            } else {
                #[cfg(test)]
                panic!("Invalid jump position");
            }
        }
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
