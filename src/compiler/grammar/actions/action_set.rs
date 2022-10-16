use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        grammar::instruction::{CompilerState, Instruction},
        lexer::{string::StringItem, tokenizer::TokenInfo, word::Word, Token},
        CompileError,
    },
    runtime::string::IntoString,
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
                        name = self.parse_variable_name(token_info)?.into();
                    } else {
                        value = self.parse_string_token(token_info)?;
                        break;
                    }
                }
            }
        }

        modifiers.sort_unstable_by(|a: &Modifier, b: &Modifier| b.cmp(a));

        self.instructions.push(Instruction::Set(Set {
            modifiers,
            name: name.unwrap(),
            value,
        }));
        Ok(())
    }

    pub(crate) fn parse_variable_name(
        &mut self,
        token_info: TokenInfo,
    ) -> Result<Variable, CompileError> {
        match token_info.token {
            Token::StringConstant(value) => Ok(self.register_variable(value.into_string())),
            _ => Err(token_info.invalid("variable name must be a constant")),
        }
    }

    pub(crate) fn register_variable(&mut self, name: String) -> Variable {
        let name = name.to_lowercase();
        match name.strip_prefix("global.") {
            Some(global_var) if !global_var.is_empty() => Variable::Global(global_var.to_string()),
            _ => {
                if !self.is_var_global(&name) {
                    Variable::Local(self.register_local_var(name))
                } else {
                    Variable::Global(name)
                }
            }
        }
    }
}

impl From<Word> for Modifier {
    fn from(word: Word) -> Self {
        match word {
            Word::Lower => Modifier::Lower,
            Word::Upper => Modifier::Upper,
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
