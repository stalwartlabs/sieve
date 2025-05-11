/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::sync::Arc;

use crate::compiler::{Number, VariableType};

pub mod parser;
pub mod tokenizer;

#[derive(Debug, PartialEq, Eq, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum Expression {
    Variable(VariableType),
    Constant(Constant),
    BinaryOperator(BinaryOperator),
    UnaryOperator(UnaryOperator),
    JmpIf { val: bool, pos: u32 },
    Function { id: u32, num_args: u32 },
    ArrayAccess,
    ArrayBuild(u32),
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
    String(Arc<String>),
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
