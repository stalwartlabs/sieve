/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::variable::Array;
use super::{
    RuntimeError, Variable,
    context::Pending,
    eval::ValueRef,
    handler::{Handler, Reply},
};
use crate::{
    Context, Sieve,
    bytecode::rec::{Range, tag},
    compiler::grammar::expr::{BinaryOperator, UnaryOperator, parser::ID_EXTERNAL},
};
use std::borrow::Cow;

impl<'x> Context<'x> {
    pub(crate) fn eval_expression<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        range: Range,
        handler: &mut H,
    ) -> Result<Option<Variable<'x>>, RuntimeError> {
        let mut pos = self.expr_pos;
        let mut iter = script.recs(Range {
            start: range.start + pos as u32,
            len: range.len.saturating_sub(pos as u32),
        })?;

        while let Some(rec) = iter.next() {
            pos += 1;
            match rec.tag {
                tag::VAR_LOCAL => {
                    let value = self.local_variable(rec.c);
                    self.expr_stack.push(value);
                }
                tag::VAR_MATCH => {
                    let value = self.match_variable(rec.b);
                    self.expr_stack.push(value);
                }
                tag::INT => {
                    self.expr_stack.push(Variable::Integer(rec.e as i64));
                }
                tag::FLOAT => {
                    self.expr_stack.push(Variable::Float(f64::from_bits(rec.e)));
                }
                tag::TEXT => {
                    self.expr_stack
                        .push(Variable::borrowed(script.str(rec.str())?));
                }
                tag::UN_OP => {
                    let value = self.expr_stack.pop().unwrap_or_default();
                    self.expr_stack.push(match UnaryOperator::from_code(rec.b) {
                        UnaryOperator::Not => value.op_not(),
                        UnaryOperator::Minus => value.op_minus(),
                    });
                }
                tag::BIN_OP => {
                    let right = self.expr_stack.pop().unwrap_or_default();
                    let left = self.expr_stack.pop().unwrap_or_default();
                    self.expr_stack
                        .push(match BinaryOperator::from_code(rec.b) {
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
                tag::CALL => {
                    let num_args = rec.c as usize;
                    let start = self.expr_stack.len().saturating_sub(num_args);
                    if let Some(fnc) = self.runtime.functions.get(rec.d as usize) {
                        let result = (fnc)(self, &self.expr_stack[start..]);
                        self.expr_stack.truncate(start);
                        let result = self.intern(result);
                        self.expr_stack.push(result);
                    } else {
                        let id = ID_EXTERNAL
                            .checked_sub(rec.d)
                            .ok_or(RuntimeError::InvalidBytecode)?;
                        let reply = handler.function(self, id, &self.expr_stack[start..]);
                        self.expr_stack.truncate(start);
                        match reply {
                            Reply::Ready(result) => {
                                let result = self.intern(result);
                                self.expr_stack.push(result);
                            }
                            Reply::Pending => {
                                self.expr_pos = pos;
                                self.pending = Pending::Function;
                                return Ok(None);
                            }
                            Reply::Error(err) => return Err(err),
                        }
                    }
                }
                tag::JMP_IF => {
                    if self.expr_stack.last().is_some_and(|v| v.to_bool()) == (rec.b != 0) {
                        pos += rec.d as usize;
                        if pos > range.len as usize {
                            return Err(RuntimeError::InvalidBytecode);
                        }
                        iter = script.recs(Range {
                            start: range.start + pos as u32,
                            len: range.len.saturating_sub(pos as u32),
                        })?;
                    }
                }
                tag::ARRAY_ACCESS => {
                    let index = self.expr_stack.pop().unwrap_or_default().to_usize();
                    let value = match self.expr_stack.pop() {
                        Some(Variable::Array(array)) => {
                            array.get(index).cloned().unwrap_or_default()
                        }
                        Some(value) if index == 0 && !value.is_empty() => value,
                        _ => Variable::default(),
                    };
                    self.expr_stack.push(value);
                }
                tag::ARRAY_BUILD => {
                    let start = self.expr_stack.len().saturating_sub(rec.d as usize);
                    let items = self.expr_stack.split_off(start);
                    self.expr_stack.push(Variable::Array(Array::Owned(items)));
                }
                _ => {
                    let value = ValueRef::decode(script, rec, &mut iter)?;
                    if matches!(value, ValueRef::Header(_)) {
                        pos += 1;
                    }
                    let value = self.variable_ref(script, value)?.unwrap_or_default();
                    self.expr_stack.push(value);
                }
            }
        }

        let result = self.expr_stack.pop().unwrap_or_default();
        self.expr_stack.clear();
        self.expr_pos = 0;
        Ok(Some(result))
    }
}

impl<'x> Variable<'x> {
    pub fn op_add(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_add(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a + b),
            (Variable::Integer(i), Variable::Float(f))
            | (Variable::Float(f), Variable::Integer(i)) => Variable::Float(i as f64 + f),
            (Variable::Array(a), Variable::Array(b)) => {
                Variable::Array(Array::Owned(a.iter().chain(b.iter()).cloned().collect()))
            }
            (Variable::Array(a), b) => {
                Variable::Array(Array::Owned(a.iter().cloned().chain([b]).collect()))
            }
            (a, Variable::Array(b)) => Variable::Array(Array::Owned(
                [a].into_iter().chain(b.iter().cloned()).collect(),
            )),
            (Variable::String(a), b) => {
                if a.is_empty() {
                    b
                } else if b.is_empty() {
                    Variable::String(a)
                } else {
                    let b = b.to_string();
                    let mut result = a.into_owned();
                    result.push_str(&b);
                    Variable::String(Cow::Owned(result))
                }
            }
            (a, Variable::String(b)) => {
                if b.is_empty() {
                    a
                } else {
                    let a = a.to_string();
                    let mut result = String::with_capacity(a.len() + b.len());
                    result.push_str(&a);
                    result.push_str(&b);
                    Variable::String(Cow::Owned(result))
                }
            }
        }
    }

    pub fn op_subtract(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_sub(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a - b),
            (Variable::Integer(a), Variable::Float(b)) => Variable::Float(a as f64 - b),
            (Variable::Float(a), Variable::Integer(b)) => Variable::Float(a - b as f64),
            (Variable::Array(a), b) | (b, Variable::Array(a)) => Variable::Array(Array::Owned(
                a.iter().filter(|v| *v != &b).cloned().collect(),
            )),
            (a, b) => a.parse_number().op_subtract(b.parse_number()),
        }
    }

    pub fn op_multiply(self, other: Variable<'x>) -> Variable<'x> {
        match (self, other) {
            (Variable::Integer(a), Variable::Integer(b)) => Variable::Integer(a.saturating_mul(b)),
            (Variable::Float(a), Variable::Float(b)) => Variable::Float(a * b),
            (Variable::Integer(i), Variable::Float(f))
            | (Variable::Float(f), Variable::Integer(i)) => Variable::Float(i as f64 * f),
            (a, b) => a.parse_number().op_multiply(b.parse_number()),
        }
    }

    pub fn op_divide(self, other: Variable<'x>) -> Variable<'x> {
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

    pub fn op_and(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self.to_bool() & other.to_bool()))
    }

    pub fn op_or(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self.to_bool() | other.to_bool()))
    }

    pub fn op_xor(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self.to_bool() ^ other.to_bool()))
    }

    pub fn op_eq(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self == other))
    }

    pub fn op_ne(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self != other))
    }

    pub fn op_lt(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self < other))
    }

    pub fn op_le(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self <= other))
    }

    pub fn op_gt(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self > other))
    }

    pub fn op_ge(self, other: Variable<'x>) -> Variable<'x> {
        Variable::Integer(i64::from(self >= other))
    }

    pub fn op_not(self) -> Variable<'x> {
        Variable::Integer(i64::from(!self.to_bool()))
    }

    pub fn op_minus(self) -> Variable<'x> {
        match self {
            Variable::Integer(n) => Variable::Integer(n.saturating_neg()),
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
            Variable::Integer(n) => Variable::Integer(*n),
            Variable::Float(n) => Variable::Float(*n),
            Variable::Array(l) => Variable::Integer(l.is_empty() as i64),
            _ => Variable::Integer(0),
        }
    }
}

impl BinaryOperator {
    #[inline(always)]
    pub(crate) fn from_code(code: u8) -> BinaryOperator {
        match code {
            0 => BinaryOperator::Add,
            1 => BinaryOperator::Subtract,
            2 => BinaryOperator::Multiply,
            3 => BinaryOperator::Divide,
            4 => BinaryOperator::And,
            5 => BinaryOperator::Or,
            6 => BinaryOperator::Xor,
            7 => BinaryOperator::Eq,
            8 => BinaryOperator::Ne,
            9 => BinaryOperator::Lt,
            10 => BinaryOperator::Le,
            11 => BinaryOperator::Gt,
            _ => BinaryOperator::Ge,
        }
    }
}

impl UnaryOperator {
    #[inline(always)]
    pub(crate) fn from_code(code: u8) -> UnaryOperator {
        match code {
            0 => UnaryOperator::Not,
            _ => UnaryOperator::Minus,
        }
    }
}
#[cfg(test)]
mod test {
    use ahash::{HashMap, HashMapExt};

    use crate::{
        compiler::{
            VariableType,
            grammar::expr::{
                BinaryOperator, Expression, Token, UnaryOperator, parser::ExpressionParser,
                tokenizer::Tokenizer,
            },
        },
        runtime::Variable,
    };

    use evalexpr::*;

    pub trait EvalExpression {
        fn eval<'a>(&self, variables: &HashMap<String, Variable<'a>>) -> Option<Variable<'a>>;
    }

    impl EvalExpression for Vec<Expression> {
        fn eval<'a>(&self, variables: &HashMap<String, Variable<'a>>) -> Option<Variable<'a>> {
            let mut stack = Vec::with_capacity(self.len());
            let mut exprs = self.iter();

            while let Some(expr) = exprs.next() {
                match expr {
                    Expression::VariableOther(v) => {
                        if let VariableType::Global(v) = v.as_ref() {
                            stack.push(variables.get(v)?.clone());
                        } else {
                            unreachable!("Invalid expression")
                        }
                    }
                    Expression::ConstantInteger(i) => stack.push(Variable::Integer(*i)),
                    Expression::ConstantFloat(f) => stack.push(Variable::Float(*f)),
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
                    if matches!(&v, Value::Float(f) if f64::is_infinite(*f)) {
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
