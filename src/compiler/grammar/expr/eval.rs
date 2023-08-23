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

use crate::{compiler::Number, Context};

use super::{BinaryOperator, Expression, UnaryOperator};

impl<'x> Context<'x> {
    pub(crate) fn eval_expression(&self, expr: &[Expression]) -> Option<Number> {
        let mut stack = Vec::with_capacity(expr.len());
        for expr in expr {
            match expr {
                Expression::Variable(v) => {
                    stack.push(self.variable(v)?.to_number());
                }
                Expression::Number(n) => {
                    stack.push(*n);
                }
                Expression::UnaryOperator(op) => {
                    let value = stack.last_mut()?;
                    *value = value.apply_unary(*op);
                }
                Expression::BinaryOperator(op) => {
                    let right = stack.pop()?;
                    let left = stack.last_mut()?;
                    *left = left.apply_binary(*op, right);
                }
            }
        }
        stack.pop()
    }
}

impl Number {
    fn apply_binary(self, op: BinaryOperator, other: Number) -> Number {
        let (a, b) = match (self, other) {
            (Number::Integer(a), Number::Integer(b)) => {
                return match op {
                    BinaryOperator::Add => Number::Integer(a.saturating_add(b)),
                    BinaryOperator::Subtract => Number::Integer(a.saturating_sub(b)),
                    BinaryOperator::Multiply => Number::Integer(a.saturating_mul(b)),
                    BinaryOperator::Divide => {
                        Number::Float(if b != 0 { a as f64 / b as f64 } else { 0.0 })
                    }
                    BinaryOperator::And => (a.into_bool() & b.into_bool()).into(),
                    BinaryOperator::Or => (a.into_bool() | b.into_bool()).into(),
                    BinaryOperator::Xor => (a.into_bool() ^ b.into_bool()).into(),
                    BinaryOperator::Eq => Number::Integer(i64::from(a == b)),
                    BinaryOperator::Ne => Number::Integer(i64::from(a != b)),
                    BinaryOperator::Lt => Number::Integer(i64::from(a < b)),
                    BinaryOperator::Le => Number::Integer(i64::from(a <= b)),
                    BinaryOperator::Gt => Number::Integer(i64::from(a > b)),
                    BinaryOperator::Ge => Number::Integer(i64::from(a >= b)),
                }
            }
            (Number::Float(a), Number::Float(b)) => (a, b),
            (Number::Integer(a), Number::Float(b)) => (a as f64, b),
            (Number::Float(a), Number::Integer(b)) => (a, b as f64),
        };

        match op {
            BinaryOperator::Add => Number::Float(a + b),
            BinaryOperator::Subtract => Number::Float(a - b),
            BinaryOperator::Multiply => Number::Float(a * b),
            BinaryOperator::Divide => Number::Float(if b != 0.0 { a / b } else { 0.0 }),
            BinaryOperator::And => (a.into_bool() & b.into_bool()).into(),
            BinaryOperator::Or => (a.into_bool() | b.into_bool()).into(),
            BinaryOperator::Xor => (a.into_bool() ^ b.into_bool()).into(),
            BinaryOperator::Eq => Number::Integer(i64::from(a == b)),
            BinaryOperator::Ne => Number::Integer(i64::from(a != b)),
            BinaryOperator::Lt => Number::Integer(i64::from(a < b)),
            BinaryOperator::Le => Number::Integer(i64::from(a <= b)),
            BinaryOperator::Gt => Number::Integer(i64::from(a > b)),
            BinaryOperator::Ge => Number::Integer(i64::from(a >= b)),
        }
    }

    fn apply_unary(self, op: UnaryOperator) -> Number {
        match op {
            UnaryOperator::Not => match self {
                Number::Integer(n) => Number::Integer(i64::from(n == 0)),
                Number::Float(n) => Number::Integer(i64::from(n == 0.0)),
            },
            UnaryOperator::Minus => match self {
                Number::Integer(n) => Number::Integer(-n),
                Number::Float(n) => Number::Float(-n),
            },
        }
    }
}

trait IntoBool {
    fn into_bool(self) -> bool;
}

impl IntoBool for f64 {
    #[inline(always)]
    fn into_bool(self) -> bool {
        self != 0.0
    }
}

impl IntoBool for i64 {
    #[inline(always)]
    fn into_bool(self) -> bool {
        self != 0
    }
}

impl From<bool> for Number {
    #[inline(always)]
    fn from(b: bool) -> Self {
        Number::Integer(i64::from(b))
    }
}

impl From<i64> for Number {
    #[inline(always)]
    fn from(n: i64) -> Self {
        Number::Integer(n)
    }
}

impl From<f64> for Number {
    #[inline(always)]
    fn from(n: f64) -> Self {
        Number::Float(n)
    }
}

impl From<i32> for Number {
    #[inline(always)]
    fn from(n: i32) -> Self {
        Number::Integer(n as i64)
    }
}
