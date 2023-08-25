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
                    stack.push(
                        self.variable(v)?
                            .to_number_checked()
                            .unwrap_or(Number::Integer(0)),
                    );
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

    pub fn is_non_zero(&self) -> bool {
        match self {
            Number::Integer(n) => *n != 0,
            Number::Float(n) => *n != 0.0,
        }
    }
}

impl Default for Number {
    fn default() -> Self {
        Number::Integer(0)
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

#[cfg(test)]
mod test {
    use ahash::{HashMap, HashMapExt};

    use crate::compiler::{
        grammar::expr::{parser::ExpressionParser, tokenizer::Tokenizer, Expression},
        Number, VariableType,
    };

    use evalexpr::*;

    pub trait EvalExpression {
        fn eval(&self, variables: &HashMap<String, Number>) -> Option<Number>;
    }

    impl EvalExpression for Vec<Expression> {
        fn eval(&self, variables: &HashMap<String, Number>) -> Option<Number> {
            let mut stack = Vec::with_capacity(self.len());
            for expr in self.iter() {
                match expr {
                    Expression::Variable(VariableType::Global(v)) => {
                        stack.push(*variables.get(v)?);
                    }
                    Expression::Number(n) => {
                        stack.push(*n);
                    }
                    Expression::UnaryOperator(op) => {
                        let value = stack.pop()?;
                        stack.push(value.apply_unary(*op));
                    }
                    Expression::BinaryOperator(op) => {
                        let right = stack.pop()?;
                        let left = stack.pop()?;
                        stack.push(left.apply_binary(*op, right));
                    }
                    _ => unreachable!("Invalid expression"),
                }
            }
            stack.pop()
        }
    }

    #[test]
    fn eval_expression() {
        let mut variables = HashMap::from_iter([
            ("A".to_string(), Number::Integer(0)),
            ("B".to_string(), Number::Integer(0)),
            ("C".to_string(), Number::Integer(0)),
            ("D".to_string(), Number::Integer(0)),
            ("E".to_string(), Number::Integer(0)),
            ("F".to_string(), Number::Integer(0)),
            ("G".to_string(), Number::Integer(0)),
            ("H".to_string(), Number::Integer(0)),
            ("I".to_string(), Number::Integer(0)),
            ("J".to_string(), Number::Integer(0)),
        ]);
        let num_vars = variables.len();

        for expr in [
            "A + B",
            "A * B",
            "A / B",
            "A - B",
            "-A",
            "A == B",
            "A != B",
            "A > B",
            "A < B",
            "A >= B",
            "A <= B",
            "A + B * C - D / E",
            "A + B + C - D - E",
            "(A + B) * (C - D) / E",
            "A - B + C * D / E * F - G",
            "A + B * C - D / E",
            "(A + B) * (C - D) / E",
            "A - B + C / D * E",
            "(A + B) / (C - D) + E",
            "A * (B + C) - D / E",
            "A / (B - C + D) * E",
            "(A + B) * C - D / (E + F)",
            "A * B - C + D / E",
            "A + B - C * D / E",
            "(A * B + C) / D - E",
            "A - B / C + D * E",
            "A + B * (C - D) / E",
            "A * B / C + (D - E)",
            "(A - B) * C / D + E",
            "A * (B / C) - D + E",
            "(A + B) / (C + D) * E",
            "A - B * C / D + E",
            "A + (B - C) * D / E",
            "(A + B) * (C / D) - E",
            "A - B / (C * D) + E",
            "(A + B) > (C - D) && E <= F",
            "A * B == C / D || E - F != G + H",
            "A / B >= C * D && E + F < G - H",
            "(A * B - C) != (D / E + F) && G > H",
            "A - B < C && D + E >= F * G",
            "(A * B) > C && (D / E) < F || G == H",
            "(A + B) <= (C - D) || E > F && G != H",
            "A * B != C + D || E - F == G / H",
            "A >= B * C && D < E - F || G != H + I",
            "(A / B + C) > D && E * F <= G - H",
            "A * (B - C) == D && E / F > G + H",
            "(A - B + C) != D || E * F >= G && H < I",
            "A < B / C && D + E * F == G - H",
            "(A + B * C) <= D && E > F / G",
            "(A * B - C) > D || E <= F + G && H != I",
            "A != B / C && D == E * F - G",
            "A <= B + C - D && E / F > G * H",
            "(A - B * C) < D || E >= F + G && H != I",
            "(A + B) / C == D && E - F < G * H",
            "A * B != C && D >= E + F / G || H < I",
            "!(A * B != C) && !(D >= E + F / G) || !(H < I)",
            "-A - B - (- C - D) - E - (-F)",
        ] {
            for (pos, v) in variables.values_mut().enumerate() {
                *v = Number::Integer(pos as i64 + 1);
            }

            assert_expr(expr, &variables);

            for (pos, v) in variables.values_mut().enumerate() {
                *v = Number::Integer((num_vars - pos) as i64);
            }

            assert_expr(expr, &variables);
        }

        for expr in [
            "true && false",
            "!true || false",
            "true && !false",
            "!(true && false)",
            "true || true && false",
            "!false && (true || false)",
            "!(true || !false) && true",
            "!(!true && !false)",
            "true || false && !true",
            "!(true && true) || !false",
            "!(!true || !false) && (!false) && !(!true)",
        ] {
            let result = parse_expression(expr.replace("true", "1").replace("false", "0").as_str())
                .eval(&HashMap::new())
                .unwrap();

            //println!("{} => {:?}", expr, result);

            match (eval(expr).expect(expr), result) {
                (Value::Float(a), Number::Float(b)) if a == b => (),
                (Value::Float(a), Number::Integer(b)) if a == b as f64 => (),
                (Value::Boolean(a), Number::Integer(b)) if a == (b != 0) => (),
                (a, b) => {
                    panic!("{} => {:?} != {:?}", expr, a, b)
                }
            }
        }
    }

    fn assert_expr(expr: &str, variables: &HashMap<String, Number>) {
        let e = parse_expression(expr);

        let result = e.eval(variables).unwrap();

        let mut str_expr = expr.to_string();
        let mut str_expr_float = expr.to_string();
        for (k, v) in variables {
            let v = v.to_string();

            if v.contains('.') {
                str_expr_float = str_expr_float.replace(k, &v);
            } else {
                str_expr_float = str_expr_float.replace(k, &format!("{}.0", v));
            }
            str_expr = str_expr.replace(k, &v);
        }

        assert_eq!(
            parse_expression(&str_expr)
                .eval(&HashMap::new())
                .unwrap()
                .to_float(),
            result.to_float()
        );

        assert_eq!(
            parse_expression(&str_expr_float)
                .eval(&HashMap::new())
                .unwrap()
                .to_float(),
            result.to_float()
        );

        //println!("{str_expr} ({e:?}) => {result:?}");

        match (
            eval(&str_expr_float)
                .map(|v| {
                    // Divisions by zero are converted to 0.0
                    if matches!(&v, Value::Float(f) if f.is_infinite()) {
                        Value::Float(0.0)
                    } else {
                        v
                    }
                })
                .expect(&str_expr),
            result,
        ) {
            (Value::Float(a), Number::Float(b)) if a == b => (),
            (Value::Float(a), Number::Integer(b)) if a == b as f64 => (),
            (Value::Boolean(a), Number::Integer(b)) if a == (b != 0) => (),
            (a, b) => {
                panic!("{} => {:?} != {:?}", str_expr, a, b)
            }
        }
    }

    fn parse_expression(expr: &str) -> Vec<Expression> {
        ExpressionParser::from_tokenizer(Tokenizer::new(expr, |var_name: &str, _: bool| {
            Ok::<_, String>(VariableType::Global(var_name.to_string()))
        }))
        .parse()
        .unwrap()
        .output
    }
}
