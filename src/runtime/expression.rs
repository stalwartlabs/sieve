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

use std::{cmp::Ordering, fmt::Display};

use crate::{compiler::Number, runtime::Variable, Context};

use crate::compiler::grammar::expr::{BinaryOperator, Constant, Expression, UnaryOperator};

impl<'x> Context<'x> {
    pub(crate) fn eval_expression<'y: 'x>(
        &'y self,
        expr: &'x [Expression],
    ) -> Option<Variable<'x>> {
        let mut stack = Vec::with_capacity(expr.len());
        let mut exprs = expr.iter();
        while let Some(expr) = exprs.next() {
            match expr {
                Expression::Variable(v) => {
                    stack.push(self.variable(v).unwrap_or_default());
                }
                Expression::Constant(val) => {
                    stack.push(Variable::from(val));
                }
                Expression::UnaryOperator(op) => {
                    let value = stack.pop()?;
                    stack.push(match op {
                        UnaryOperator::Not => value.op_not(),
                        UnaryOperator::Minus => value.op_minus(),
                    });
                }
                Expression::BinaryOperator(op) => {
                    let right = stack.pop()?;
                    let left = stack.pop()?;
                    stack.push(match op {
                        BinaryOperator::Add => left.op_add(right),
                        BinaryOperator::Subtract => left.op_subtract(right),
                        BinaryOperator::Multiply => left.op_multiply(right),
                        BinaryOperator::Divide => left.op_divide(right),
                        BinaryOperator::And => left.op_and(right),
                        BinaryOperator::Or => left.op_or(right),
                        BinaryOperator::Xor => left.op_xor(right),
                        BinaryOperator::Eq => left.op_eq(right),
                        BinaryOperator::Ne => left.op_ne(right),
                        BinaryOperator::Lt => left.op_lt(right),
                        BinaryOperator::Le => left.op_le(right),
                        BinaryOperator::Gt => left.op_gt(right),
                        BinaryOperator::Ge => left.op_ge(right),
                    });
                }
                Expression::Function { id, num_args } => {
                    let num_args = *num_args as usize;
                    let mut args = vec![Variable::Integer(0); num_args];
                    for arg_num in 0..num_args {
                        args[num_args - arg_num - 1] = stack.pop()?;
                    }
                    stack.push((self.runtime.functions.get(*id as usize)?)(self, args));
                }
                Expression::JmpIf { val, pos } => {
                    if stack.last()?.to_bool() == *val {
                        for _ in 0..*pos {
                            exprs.next();
                        }
                    }
                }
                Expression::ArrayAccess => {
                    let index = stack.pop()?.to_usize();
                    let mut array = stack.pop()?.into_array();
                    stack.push(if index < array.len() {
                        array.remove(index)
                    } else {
                        Variable::default()
                    });
                }
            }
        }
        stack.pop()
    }
}

impl<'x> Variable<'x> {
    fn op_add(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_add(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a + b),
            (Variable::Integer(i), Variable::Float(f))
            | (Variable::Float(f), Variable::Integer(i)) => Variable::Float(i as f64 + f),
            (Variable::Array(mut a), Variable::Array(b)) => {
                a.extend(b);
                Variable::Array(a)
            }
            (Variable::ArrayRef(a), Variable::ArrayRef(b)) => {
                Variable::Array(a.iter().chain(b).map(|v| v.as_ref()).collect())
            }
            (Variable::Array(mut a), Variable::ArrayRef(b)) => {
                a.extend(b.iter().map(|v| v.as_ref()));
                Variable::Array(a)
            }
            (Variable::ArrayRef(a), Variable::Array(b)) => {
                Variable::Array(a.iter().map(|v| v.as_ref()).chain(b).collect())
            }
            (Variable::Array(mut a), b) => {
                a.push(b);
                Variable::Array(a)
            }
            (Variable::ArrayRef(a), b) => {
                Variable::Array(a.iter().map(|v| v.as_ref()).chain([b]).collect())
            }
            (a, Variable::Array(mut b)) => {
                b.insert(0, a);
                Variable::Array(b)
            }
            (a, Variable::ArrayRef(b)) => Variable::Array(
                [a].into_iter()
                    .chain(b.iter().map(|v| v.as_ref()))
                    .collect(),
            ),
            (Variable::String(a), b) => {
                if !a.is_empty() {
                    Variable::String(format!("{}{}", a, b))
                } else {
                    b
                }
            }
            (a, Variable::String(b)) => {
                if !b.is_empty() {
                    Variable::String(format!("{}{}", a, b))
                } else {
                    a
                }
            }
            (Variable::StringRef(a), b) => {
                if !a.is_empty() {
                    Variable::String(format!("{}{}", a, b))
                } else {
                    b
                }
            }
            (a, Variable::StringRef(b)) => {
                if !b.is_empty() {
                    Variable::String(format!("{}{}", a, b))
                } else {
                    a
                }
            }
        }
    }

    fn op_subtract(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_sub(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a - b),
            (Variable::Integer(a), Variable::Float(b)) => Variable::Float(a as f64 - b),
            (Variable::Float(a), Variable::Integer(b)) => Variable::Float(a - b as f64),
            (Variable::Array(mut a), b) | (b, Variable::Array(mut a)) => {
                a.retain(|v| *v != b);
                Variable::Array(a)
            }
            (a, b) => a.parse_number().op_subtract(b.parse_number()),
        }
    }

    fn op_multiply(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_mul(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a * b),
            (Variable::Integer(i), Variable::Float(f))
            | (Variable::Float(f), Variable::Integer(i)) => Variable::Float(i as f64 * f),
            (a, b) => a.parse_number().op_multiply(b.parse_number()),
        }
    }

    fn op_divide(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => {
                Variable::Float(if b != 0 { a as f64 / b as f64 } else { 0.0 })
            }
            (Variable::Float(a), Variable::Float(b)) => {
                Variable::Float(if b != 0.0 { a / b } else { 0.0 })
            }
            (Variable::Integer(a), Variable::Float(b)) => {
                Variable::Float(if b != 0.0 { a as f64 / b } else { 0.0 })
            }
            (Variable::Float(a), Variable::Integer(b)) => {
                Variable::Float(if b != 0 { a / b as f64 } else { 0.0 })
            }
            (a, b) => a.parse_number().op_divide(b.parse_number()),
        }
    }

    fn op_and(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self.to_bool() & other.to_bool()))
    }

    fn op_or(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self.to_bool() | other.to_bool()))
    }

    fn op_xor(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self.to_bool() ^ other.to_bool()))
    }

    fn op_eq(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self == other))
    }

    fn op_ne(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self != other))
    }

    fn op_lt(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self < other))
    }

    fn op_le(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self <= other))
    }

    fn op_gt(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self > other))
    }

    fn op_ge(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self >= other))
    }

    fn op_not(self) -> Variable<'x> {
        Variable::Integer(i64::from(!self.to_bool()))
    }

    fn op_minus(self) -> Variable<'x> {
        match self {
            Variable::Integer(n) => Variable::Integer(-n),
            Variable::Float(n) => Variable::Float(-n),
            _ => self.parse_number().op_minus(),
        }
    }

    pub fn parse_number(&self) -> Variable<'x> {
        match self {
            Variable::String(s) if !s.is_empty() => {
                if let Ok(n) = s.parse::<i64>() {
                    Variable::Integer(n)
                } else if let Ok(n) = s.parse::<f64>() {
                    Variable::Float(n)
                } else {
                    Variable::Integer(0)
                }
            }
            Variable::StringRef(s) if !s.is_empty() => {
                if let Ok(n) = s.parse::<i64>() {
                    Variable::Integer(n)
                } else if let Ok(n) = s.parse::<f64>() {
                    Variable::Float(n)
                } else {
                    Variable::Integer(0)
                }
            }
            Variable::Integer(n) => Variable::Integer(*n),
            Variable::Float(n) => Variable::Float(*n),
            Variable::Array(l) => Variable::Integer(l.is_empty() as i64),
            _ => Variable::Integer(0),
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Variable::Float(f) => *f != 0.0,
            Variable::Integer(n) => *n != 0,
            Variable::String(s) => !s.is_empty(),
            Variable::StringRef(s) => !s.is_empty(),
            Variable::Array(a) => !a.is_empty(),
            Variable::ArrayRef(a) => !a.is_empty(),
        }
    }
}

impl<'x> PartialEq for Variable<'x> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Float(b)) | (Self::Float(b), Self::Integer(a)) => {
                *a as f64 == *b
            }
            (Self::String(a), Self::String(b)) => a == b,
            (Self::StringRef(a), Self::StringRef(b)) => a == b,
            (Self::String(a), Self::StringRef(b)) | (Self::StringRef(b), Self::String(a)) => a == b,
            (Self::String(_) | Self::StringRef(_), Self::Integer(_) | Self::Float(_)) => {
                &self.parse_number() == other
            }
            (Self::Integer(_) | Self::Float(_), Self::String(_) | Self::StringRef(_)) => {
                self == &other.parse_number()
            }
            (Self::Array(a), Self::Array(b)) => a == b,
            (Self::ArrayRef(a), Self::ArrayRef(b)) => a == b,
            (Self::Array(a), Self::ArrayRef(b)) | (Self::ArrayRef(b), Self::Array(a)) => a == *b,
            _ => false,
        }
    }
}

impl Eq for Variable<'_> {}

impl<'x> PartialOrd for Variable<'x> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a.partial_cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            (Self::Integer(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            (Self::Float(a), Self::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => a.partial_cmp(b),
            (Self::StringRef(a), Self::StringRef(b)) => a.partial_cmp(b),
            (Self::String(a), Self::StringRef(b)) => a.as_str().partial_cmp(b),
            (Self::StringRef(a), Self::String(b)) => a.partial_cmp(&b.as_str()),
            (Self::String(_) | Self::StringRef(_), Self::Integer(_) | Self::Float(_)) => {
                self.parse_number().partial_cmp(other)
            }
            (Self::Integer(_) | Self::Float(_), Self::String(_) | Self::StringRef(_)) => {
                self.partial_cmp(&other.parse_number())
            }
            (Self::Array(a), Self::Array(b)) => a.partial_cmp(b),
            (Self::ArrayRef(a), Self::ArrayRef(b)) => a.partial_cmp(b),
            (Self::Array(a), Self::ArrayRef(b)) => a.partial_cmp(b),
            (Self::ArrayRef(a), Self::Array(b)) => a.partial_cmp(&b),
            (Self::Array(_) | Self::ArrayRef(_) | Self::String(_) | Self::StringRef(_), _) => {
                Ordering::Greater.into()
            }
            (_, Self::Array(_) | Self::ArrayRef(_)) => Ordering::Less.into(),
        }
    }
}

impl<'x> Ord for Variable<'x> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Greater)
    }
}

impl Display for Variable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variable::String(v) => v.fmt(f),
            Variable::StringRef(v) => v.fmt(f),
            Variable::Integer(v) => v.fmt(f),
            Variable::Float(v) => v.fmt(f),
            Variable::Array(v) => {
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    v.fmt(f)?;
                }
                Ok(())
            }
            Variable::ArrayRef(v) => {
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    v.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

impl Number {
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

impl<'x> From<&'x Constant> for Variable<'x> {
    fn from(value: &'x Constant) -> Self {
        match value {
            Constant::Integer(i) => Variable::Integer(*i),
            Constant::Float(f) => Variable::Float(*f),
            Constant::String(s) => Variable::StringRef(s.as_str()),
            Constant::Array(a) => Variable::Array(a.iter().map(|v| v.into()).collect()),
        }
    }
}

#[cfg(test)]
mod test {
    use ahash::{HashMap, HashMapExt};

    use crate::{
        compiler::{
            grammar::expr::{
                parser::ExpressionParser, tokenizer::Tokenizer, BinaryOperator, Expression, Token,
                UnaryOperator,
            },
            VariableType,
        },
        runtime::Variable,
    };

    use evalexpr::*;

    pub trait EvalExpression {
        fn eval(&self, variables: &HashMap<String, Variable>) -> Option<Variable>;
    }

    impl EvalExpression for Vec<Expression> {
        fn eval(&self, variables: &HashMap<String, Variable>) -> Option<Variable> {
            let mut stack = Vec::with_capacity(self.len());
            let mut exprs = self.iter();

            while let Some(expr) = exprs.next() {
                match expr {
                    Expression::Variable(VariableType::Global(v)) => {
                        stack.push(variables.get(v)?.as_ref().into_owned());
                    }
                    Expression::Constant(val) => {
                        stack.push(Variable::from(val));
                    }
                    Expression::UnaryOperator(op) => {
                        let value = stack.pop()?;
                        stack.push(match op {
                            UnaryOperator::Not => value.op_not(),
                            UnaryOperator::Minus => value.op_minus(),
                        });
                    }
                    Expression::BinaryOperator(op) => {
                        let right = stack.pop()?;
                        let left = stack.pop()?;
                        stack.push(match op {
                            BinaryOperator::Add => left.op_add(right),
                            BinaryOperator::Subtract => left.op_subtract(right),
                            BinaryOperator::Multiply => left.op_multiply(right),
                            BinaryOperator::Divide => left.op_divide(right),
                            BinaryOperator::And => left.op_and(right),
                            BinaryOperator::Or => left.op_or(right),
                            BinaryOperator::Xor => left.op_xor(right),
                            BinaryOperator::Eq => left.op_eq(right),
                            BinaryOperator::Ne => left.op_ne(right),
                            BinaryOperator::Lt => left.op_lt(right),
                            BinaryOperator::Le => left.op_le(right),
                            BinaryOperator::Gt => left.op_gt(right),
                            BinaryOperator::Ge => left.op_ge(right),
                        });
                    }
                    Expression::JmpIf { val, pos } => {
                        if stack.last()?.to_bool() == *val {
                            for _ in 0..*pos {
                                exprs.next();
                            }
                        }
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
            ("A".to_string(), Variable::Integer(0)),
            ("B".to_string(), Variable::Integer(0)),
            ("C".to_string(), Variable::Integer(0)),
            ("D".to_string(), Variable::Integer(0)),
            ("E".to_string(), Variable::Integer(0)),
            ("F".to_string(), Variable::Integer(0)),
            ("G".to_string(), Variable::Integer(0)),
            ("H".to_string(), Variable::Integer(0)),
            ("I".to_string(), Variable::Integer(0)),
            ("J".to_string(), Variable::Integer(0)),
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
            println!("Testing {}", expr);
            for (pos, v) in variables.values_mut().enumerate() {
                *v = Variable::Integer(pos as i64 + 1);
            }

            assert_expr(expr, &variables);

            for (pos, v) in variables.values_mut().enumerate() {
                *v = Variable::Integer((num_vars - pos) as i64);
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
            let pexp = parse_expression(expr.replace("true", "1").replace("false", "0").as_str());
            let result = pexp.eval(&HashMap::new()).unwrap();

            //println!("{} => {:?}", expr, result);

            match (eval(expr).expect(expr), result) {
                (Value::Float(a), Variable::Float(b)) if a == b => (),
                (Value::Float(a), Variable::Integer(b)) if a == b as f64 => (),
                (Value::Boolean(a), Variable::Integer(b)) if a == (b != 0) => (),
                (a, b) => {
                    panic!("{} => {:?} != {:?}", expr, a, b)
                }
            }
        }
    }

    fn assert_expr(expr: &str, variables: &HashMap<String, Variable>) {
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
                .to_number()
                .to_float(),
            result.to_number().to_float()
        );

        assert_eq!(
            parse_expression(&str_expr_float)
                .eval(&HashMap::new())
                .unwrap()
                .to_number()
                .to_float(),
            result.to_number().to_float()
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
            (Value::Float(a), Variable::Float(b)) if a == b => (),
            (Value::Float(a), Variable::Integer(b)) if a == b as f64 => (),
            (Value::Boolean(a), Variable::Integer(b)) if a == (b != 0) => (),
            (a, b) => {
                panic!("{} => {:?} != {:?}", str_expr, a, b)
            }
        }
    }

    fn parse_expression(expr: &str) -> Vec<Expression> {
        ExpressionParser::from_tokenizer(Tokenizer::new(expr, |var_name: &str, _: bool| {
            Ok::<_, String>(Token::Variable(VariableType::Global(var_name.to_string())))
        }))
        .parse()
        .unwrap()
        .output
    }
}
