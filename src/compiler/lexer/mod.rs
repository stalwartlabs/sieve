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

pub mod string;
pub mod tokenizer;
pub mod word;

use std::{borrow::Cow, fmt::Display};

use self::word::Word;

use super::{Number, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Token {
    CurlyOpen,
    CurlyClose,
    BracketOpen,
    BracketClose,
    ParenthesisOpen,
    ParenthesisClose,
    Comma,
    Semicolon,
    StringConstant(StringConstant),
    StringVariable(Vec<u8>),
    Number(usize),
    Identifier(Word),
    Tag(Word),
    Unknown(String),
    Colon,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StringConstant {
    String(String),
    Number(Number),
}

impl StringConstant {
    pub fn to_string(&self) -> Cow<str> {
        match self {
            StringConstant::String(s) => s.as_str().into(),
            StringConstant::Number(n) => n.to_string().into(),
        }
    }

    pub fn into_string(self) -> String {
        match self {
            StringConstant::String(s) => s,
            StringConstant::Number(n) => n.to_string(),
        }
    }
}

impl From<StringConstant> for Value {
    fn from(value: StringConstant) -> Self {
        match value {
            StringConstant::String(s) => Value::Text(s),
            StringConstant::Number(n) => Value::Number(n),
        }
    }
}

impl Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::CurlyOpen => f.write_str("{"),
            Token::CurlyClose => f.write_str("}"),
            Token::BracketOpen => f.write_str("["),
            Token::BracketClose => f.write_str("]"),
            Token::ParenthesisOpen => f.write_str("("),
            Token::ParenthesisClose => f.write_str(")"),
            Token::Comma => f.write_str(","),
            Token::Semicolon => f.write_str(";"),
            Token::Colon => f.write_str(":"),
            Token::Number(n) => write!(f, "{n}"),
            Token::Identifier(w) => w.fmt(f),
            Token::Tag(t) => write!(f, ":{t}"),
            Token::Unknown(s) => f.write_str(s),
            Token::StringVariable(s) => f.write_str(&String::from_utf8_lossy(s)),
            Token::StringConstant(c) => match c {
                StringConstant::String(s) => f.write_str(s),
                StringConstant::Number(n) => write!(f, "{n}"),
            },
        }
    }
}
