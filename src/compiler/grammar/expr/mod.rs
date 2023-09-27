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

use serde::{Deserialize, Serialize};

use crate::compiler::{Number, VariableType};

pub mod parser;
pub mod tokenizer;

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
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

#[derive(Debug, PartialEq, Clone, Serialize, Deserialize)]
pub(crate) enum Constant {
    Integer(i64),
    Float(f64),
    String(String),
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
        Constant::String(value)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
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

#[derive(Debug, PartialEq, Eq, Clone, Copy, Serialize, Deserialize)]
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
