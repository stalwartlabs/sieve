pub mod string;
pub mod tokenizer;
pub mod word;

use std::fmt::Display;

use crate::runtime::StringItem;

use self::word::Word;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Token {
    CurlyOpen,
    CurlyClose,
    BracketOpen,
    BracketClose,
    ParenthesisOpen,
    ParenthesisClose,
    Comma,
    Semicolon,
    String(StringItem),
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
            Token::ParenthesisOpen => f.write_str("{"),
            Token::ParenthesisClose => f.write_str("}"),
            Token::Comma => f.write_str(","),
            Token::Semicolon => f.write_str(";"),
            Token::String(s) => write!(f, "\"{}\"", s),
            Token::Number(n) => write!(f, "{}", n),
            Token::Identifier(w) => w.fmt(f),
            Token::Tag(t) => write!(f, ":{}", t),
            Token::Invalid(s) => f.write_str(s),
        }
    }
}
