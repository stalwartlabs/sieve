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

use std::fmt::Display;

use self::word::Word;

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
    StringConstant(Vec<u8>),
    StringVariable(Vec<u8>),
    Number(usize),
    Identifier(Word),
    Tag(Word),
    Invalid(String),
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
            Token::Number(n) => write!(f, "{n}"),
            Token::Identifier(w) => w.fmt(f),
            Token::Tag(t) => write!(f, ":{t}"),
            Token::Invalid(s) => f.write_str(s),
            Token::StringConstant(s) | Token::StringVariable(s) => {
                f.write_str(&String::from_utf8_lossy(s))
            }
        }
    }
}
