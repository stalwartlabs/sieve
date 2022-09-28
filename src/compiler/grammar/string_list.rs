use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, Token},
        CompileError,
    },
    runtime::StringItem,
};

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_strings(
        &mut self,
        is_matches: bool,
    ) -> Result<Vec<StringItem>, CompileError> {
        let token_info = self.unwrap_next()?;
        match token_info.token {
            Token::BracketOpen => self.parse_string_list(is_matches),
            Token::String(string) => Ok(vec![if !is_matches {
                string
            } else {
                string.into_matches()
            }]),
            _ => Err(token_info.expected("'[' or string")),
        }
    }

    pub(crate) fn parse_string_list(
        &mut self,
        is_matches: bool,
    ) -> Result<Vec<StringItem>, CompileError> {
        let mut strings = Vec::new();
        loop {
            let token_info = self.unwrap_next()?;
            match token_info.token {
                Token::String(string) => {
                    strings.push(if !is_matches {
                        string
                    } else {
                        string.into_matches()
                    });
                }
                Token::Comma => (),
                Token::BracketClose if !strings.is_empty() => break,
                _ => return Err(token_info.expected("string")),
            }
        }
        Ok(strings)
    }
}
