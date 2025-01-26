/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::{cmp::Ordering, fmt::Display};

use crate::compiler::grammar::expr::parser::ID_EXTERNAL;
use crate::Event;
use crate::{compiler::Number, runtime::Variable, Context};

use crate::compiler::grammar::expr::{BinaryOperator, Constant, Expression, UnaryOperator};

impl Context<'_> {
    pub(crate) fn eval_expression(&mut self, expr: &[Expression]) -> Result<Variable, Event> {
        let mut exprs = expr.iter().skip(self.expr_pos);
        while let Some(expr) = exprs.next() {
            self.expr_pos += 1;
            match expr {
                Expression::Variable(v) => {
                    self.expr_stack.push(self.variable(v).unwrap_or_default());
                }
                Expression::Constant(val) => {
                    self.expr_stack.push(Variable::from(val));
                }
                Expression::UnaryOperator(op) => {
                    let value = self.expr_stack.pop().unwrap_or_default();
                    self.expr_stack.push(match op {
                        UnaryOperator::Not => value.op_not(),
                        UnaryOperator::Minus => value.op_minus(),
                    });
                }
                Expression::BinaryOperator(op) => {
                    let right = self.expr_stack.pop().unwrap_or_default();
                    let left = self.expr_stack.pop().unwrap_or_default();
                    self.expr_stack.push(match op {
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

                    if let Some(fnc) = self.runtime.functions.get(*id as usize) {
                        let mut arguments = vec![Variable::Integer(0); num_args];
                        for arg_num in 0..num_args {
                            arguments[num_args - arg_num - 1] =
                                self.expr_stack.pop().unwrap_or_default();
                        }
                        self.expr_stack.push((fnc)(self, arguments));
                    } else {
                        let mut arguments = vec![Variable::Integer(0); num_args];
                        for arg_num in 0..num_args {
                            arguments[num_args - arg_num - 1] =
                                self.expr_stack.pop().unwrap_or_default();
                        }
                        self.pos -= 1; // We need to re-evaluate the function call
                        return Err(Event::Function {
                            id: ID_EXTERNAL - *id,
                            arguments,
                        });
                    }
                }
                Expression::JmpIf { val, pos } => {
                    if self.expr_stack.last().is_some_and(|v| v.to_bool()) == *val {
                        self.expr_pos += *pos as usize;
                        for _ in 0..*pos {
                            exprs.next();
                        }
                    }
                }
                Expression::ArrayAccess => {
                    let index = self.expr_stack.pop().unwrap_or_default().to_usize();
                    let array = self.expr_stack.pop().unwrap_or_default().into_array();
                    self.expr_stack
                        .push(array.get(index).cloned().unwrap_or_default());
                }
                Expression::ArrayBuild(num_items) => {
                    let num_items = *num_items as usize;
                    let mut items = vec![Variable::Integer(0); num_items];
                    for arg_num in 0..num_items {
                        items[num_items - arg_num - 1] = self.expr_stack.pop().unwrap_or_default();
                    }
                    self.expr_stack.push(Variable::Array(items.into()));
                }
            }
        }

        let result = self.expr_stack.pop().unwrap_or_default();
        self.expr_stack.clear();
        self.expr_pos = 0;
        Ok(result)
    }
}

impl Variable {
    pub fn op_add(self, other: Variable) -> Variable {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_add(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a + b),
            (Variable::Integer(i), Variable::Float(f))
            | (Variable::Float(f), Variable::Integer(i)) => Variable::Float(i as f64 + f),
            (Variable::Array(a), Variable::Array(b)) => {
                Variable::Array(a.iter().chain(b.iter()).cloned().collect::<Vec<_>>().into())
            }
            (Variable::Array(a), b) => a.iter().cloned().chain([b]).collect::<Vec<_>>().into(),
            (a, Variable::Array(b)) => [a]
                .into_iter()
                .chain(b.iter().cloned())
                .collect::<Vec<_>>()
                .into(),
            (Variable::String(a), b) => {
                if !a.is_empty() {
                    Variable::String(format!("{}{}", a, b).into())
                } else {
                    b
                }
            }
            (a, Variable::String(b)) => {
                if !b.is_empty() {
                    Variable::String(format!("{}{}", a, b).into())
                } else {
                    a
                }
            }
        }
    }

    pub fn op_subtract(self, other: Variable) -> Variable {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_sub(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a - b),
            (Variable::Integer(a), Variable::Float(b)) => Variable::Float(a as f64 - b),
            (Variable::Float(a), Variable::Integer(b)) => Variable::Float(a - b as f64),
            (Variable::Array(a), b) | (b, Variable::Array(a)) => Variable::Array(
                a.iter()
                    .filter(|v| *v != &b)
                    .cloned()
                    .collect::<Vec<_>>()
                    .into(),
            ),
            (a, b) => a.parse_number().op_subtract(b.parse_number()),
        }
    }

    pub fn op_multiply(self, other: Variable) -> Variable {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_mul(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a * b),
            (Variable::Integer(i), Variable::Float(f))
            | (Variable::Float(f), Variable::Integer(i)) => Variable::Float(i as f64 * f),
            (a, b) => a.parse_number().op_multiply(b.parse_number()),
        }
    }

    pub fn op_divide(self, other: Variable) -> Variable {
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

    pub fn op_and(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self.to_bool() & other.to_bool()))
    }

    pub fn op_or(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self.to_bool() | other.to_bool()))
    }

    pub fn op_xor(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self.to_bool() ^ other.to_bool()))
    }

    pub fn op_eq(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self == other))
    }

    pub fn op_ne(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self != other))
    }

    pub fn op_lt(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self < other))
    }

    pub fn op_le(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self <= other))
    }

    pub fn op_gt(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self > other))
    }

    pub fn op_ge(self, other: Variable) -> Variable {
        Variable::Integer(i64::from(self >= other))
    }

    pub fn op_not(self) -> Variable {
        Variable::Integer(i64::from(!self.to_bool()))
    }

    pub fn op_minus(self) -> Variable {
        match self {
            Variable::Integer(n) => Variable::Integer(-n),
            Variable::Float(n) => Variable::Float(-n),
            _ => self.parse_number().op_minus(),
        }
    }

    pub fn parse_number(&self) -> Variable {
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
            Variable::Array(a) => !a.is_empty(),
        }
    }
}

impl PartialEq for Variable {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Float(b)) | (Self::Float(b), Self::Integer(a)) => {
                *a as f64 == *b
            }
            (Self::String(a), Self::String(b)) => a == b,
            (Self::String(_), Self::Integer(_) | Self::Float(_)) => &self.parse_number() == other,
            (Self::Integer(_) | Self::Float(_), Self::String(_)) => self == &other.parse_number(),
            (Self::Array(a), Self::Array(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Variable {}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Variable {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a.partial_cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            (Self::Integer(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            (Self::Float(a), Self::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => a.partial_cmp(b),
            (Self::String(_), Self::Integer(_) | Self::Float(_)) => {
                self.parse_number().partial_cmp(other)
            }
            (Self::Integer(_) | Self::Float(_), Self::String(_)) => {
                self.partial_cmp(&other.parse_number())
            }
            (Self::Array(a), Self::Array(b)) => a.partial_cmp(b),
            (Self::Array(_) | Self::String(_), _) => Ordering::Greater.into(),
            (_, Self::Array(_)) => Ordering::Less.into(),
        }
    }
}

impl Ord for Variable {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Greater)
    }
}

impl Display for Variable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variable::String(v) => v.fmt(f),
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

impl<'x> From<&'x Constant> for Variable {
    fn from(value: &'x Constant) -> Self {
        match value {
            Constant::Integer(i) => Variable::Integer(*i),
            Constant::Float(f) => Variable::Float(*f),
            Constant::String(s) => Variable::String(s.clone()),
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
                        stack.push(variables.get(v)?.clone());
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
