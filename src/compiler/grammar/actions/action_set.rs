use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::command::{Command, CompilerState},
    lexer::{string::StringItem, tokenizer::TokenInfo, word::Word, Token},
    CompileError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) enum Modifier {
    Lower = 41,
    Upper = 40,
    LowerFirst = 31,
    UpperFirst = 30,
    QuoteWildcard = 20,
    QuoteRegex = 21,
    EncodeUrl = 15,
    Length = 10,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Set {
    pub modifiers: Vec<Modifier>,
    pub name: Variable,
    pub value: StringItem,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Variable {
    Local(usize),
    Global(String),
}

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_set(&mut self) -> Result<(), CompileError> {
        let mut modifiers = Vec::new();
        let mut name = None;
        let value;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Lower
                    | Word::Upper
                    | Word::LowerFirst
                    | Word::UpperFirst
                    | Word::QuoteWildcard
                    | Word::QuoteRegex
                    | Word::Length
                    | Word::EncodeUrl),
                ) => {
                    let modifier = word.into();
                    if !modifiers.contains(&modifier) {
                        modifiers.push(modifier);
                    }
                }
                _ => {
                    if name.is_none() {
                        match token_info.token {
                            Token::StringConstant(value) => {
                                name = if value.len() > 7
                                    && value[..7].eq_ignore_ascii_case(b"global.")
                                {
                                    Variable::Global(
                                        String::from_utf8(value[7..].to_vec()).map_err(|_| {
                                            TokenInfo {
                                                token: Token::StringConstant(value),
                                                line_num: token_info.line_num,
                                                line_pos: token_info.line_pos,
                                            }
                                            .invalid_utf8()
                                        })?,
                                    )
                                } else {
                                    let name = String::from_utf8(value).map_err(|err| {
                                        TokenInfo {
                                            token: Token::StringConstant(err.into_bytes()),
                                            line_num: token_info.line_num,
                                            line_pos: token_info.line_pos,
                                        }
                                        .invalid_utf8()
                                    })?;

                                    if !self.is_var_global(&name) {
                                        Variable::Local(self.register_local_var(&name))
                                    } else {
                                        Variable::Global(name.to_ascii_lowercase())
                                    }
                                }
                                .into();
                            }
                            _ => {
                                return Err(token_info.invalid("variable name must be a constant"));
                            }
                        }
                    } else {
                        value = self.parse_string_token(token_info)?;
                        break;
                    }
                }
            }
        }

        modifiers.sort_unstable_by(|a: &Modifier, b: &Modifier| b.cmp(a));

        self.commands.push(Command::Set(Set {
            modifiers,
            name: name.unwrap(),
            value,
        }));
        Ok(())
    }
}

impl From<Word> for Modifier {
    fn from(word: Word) -> Self {
        match word {
            Word::Lower => Modifier::Lower,
            Word::Under => Modifier::Upper,
            Word::LowerFirst => Modifier::LowerFirst,
            Word::UpperFirst => Modifier::UpperFirst,
            Word::QuoteWildcard => Modifier::QuoteWildcard,
            Word::QuoteRegex => Modifier::QuoteRegex,
            Word::Length => Modifier::Length,
            Word::EncodeUrl => Modifier::EncodeUrl,
            _ => unreachable!(),
        }
    }
}
