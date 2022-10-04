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
            Token::Number(n) => write!(f, "{}", n),
            Token::Identifier(w) => w.fmt(f),
            Token::Tag(t) => write!(f, ":{}", t),
            Token::Invalid(s) => f.write_str(s),
            Token::StringConstant(s) | Token::StringVariable(s) => {
                f.write_str(&String::from_utf8_lossy(s))
            }
        }
    }
}
