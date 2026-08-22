/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{ConstantId, Number, VariableType};

pub mod parser;
pub mod tokenizer;

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub(crate) enum Expression {
    VariableLocal(u16) = 0,
    VariableMatch(u8) = 1,
    VariableOther(Box<VariableType>) = 2,
    ConstantInteger(i64) = 3,
    ConstantFloat(f64) = 4,
    ConstantString(ConstantId) = 5,
    BinaryOperator(BinaryOperator) = 6,
    UnaryOperator(UnaryOperator) = 7,
    JmpIf { val: bool, pos: u32 } = 8,
    Function { id: u32, num_args: u32 } = 9,
    ArrayAccess = 10,
    ArrayBuild(u32) = 11,
}

impl Eq for Expression {}

impl Expression {
    pub(crate) fn variable(variable: VariableType) -> Self {
        match variable {
            VariableType::Local(id) => Expression::VariableLocal(id),
            VariableType::Match(id) => Expression::VariableMatch(id),
            other => Expression::VariableOther(Box::new(other)),
        }
    }

    pub(crate) fn number(number: Number) -> Self {
        match number {
            Number::Integer(i) => Expression::ConstantInteger(i),
            Number::Float(f) => Expression::ConstantFloat(f),
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub(crate) enum BinaryOperator {
    Add = 0,
    Subtract = 1,
    Multiply = 2,
    Divide = 3,

    And = 4,
    Or = 5,
    Xor = 6,

    Eq = 7,
    Ne = 8,
    Lt = 9,
    Le = 10,
    Gt = 11,
    Ge = 12,
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[repr(u8)]
pub(crate) enum UnaryOperator {
    Not = 0,
    Minus = 1,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum Token {
    Variable(VariableType),
    Function {
        name: String,
        id: u32,
        num_args: u32,
    },
    Number(Number),
    String(String),
    BinaryOperator(BinaryOperator),
    UnaryOperator(UnaryOperator),
    OpenParen,
    CloseParen,
    OpenBracket,
    CloseBracket,
    Comma,
}
