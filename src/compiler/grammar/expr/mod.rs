/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{ConstantId, Number, VariableType};
use std::sync::Arc;

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
pub(crate) enum Expression {
    VariableLocal(u16),
    VariableMatch(u8),
    VariableOther(Box<VariableType>),
    ConstantInteger(i64),
    ConstantFloat(f64),
    ConstantString(ConstantId),
    BinaryOperator(BinaryOperator),
    UnaryOperator(UnaryOperator),
    JmpIf { val: bool, pos: u32 },
    Function { id: u32, num_args: u32 },
    ArrayAccess,
    ArrayBuild(u32),
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

    pub(crate) fn constant(constant: Constant) -> Self {
        match constant {
            Constant::Integer(i) => Expression::ConstantInteger(i),
            Constant::Float(f) => Expression::ConstantFloat(f),
            Constant::String(_) => {
                debug_assert!(false, "String constants must be interned by the caller.");
                Expression::ConstantString(ConstantId::UNRESOLVED)
            }
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum Constant {
    Integer(i64),
    Float(f64),
    String(Arc<str>),
}

impl Eq for Constant {}

impl From<Number> for Constant {
    fn from(value: Number) -> Self {
        match value {
            Number::Integer(i) => Constant::Integer(i),
            Number::Float(f) => Constant::Float(f),
        }
    }
}

impl From<String> for Constant {
    fn from(value: String) -> Self {
        Constant::String(value.into())
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
pub(crate) enum BinaryOperator {
    Add,
    Subtract,
    Multiply,
    Divide,

    And,
    Or,
    Xor,

    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
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
pub(crate) enum UnaryOperator {
    Not,
    Minus,
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
