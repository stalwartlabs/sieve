use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError, ErrorType,
    },
    runtime::StringItem,
};

use super::{
    ast::Command, test_address::TestAddress, test_envelope::TestEnvelope, test_header::TestHeader,
    test_size::TestSize,
};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct If {
    pub test: Test,
    pub commands: Vec<Command>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Test {
    AllOf(Vec<Test>),
    AnyOf(Vec<Test>),
    Not(Box<Test>),
    True,
    False,
    Address(TestAddress),
    Envelope(TestEnvelope),
    Exists(Vec<StringItem>),
    Header(TestHeader),
    Size(TestSize),
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test(&mut self) -> Result<Test, CompileError> {
        let mut is_not = false;
        let mut tests_stack: Vec<(Vec<Test>, Word, bool)> = Vec::new();

        loop {
            let token_info = self.unwrap_next()?;
            let mut test = match token_info.token {
                Token::Comma if !tests_stack.is_empty() => {
                    if !is_not {
                        continue;
                    } else {
                        return Err(token_info.expected("test name"));
                    }
                }
                Token::ParenthesisClose if !tests_stack.is_empty() => {
                    let (tests, test_name, prev_is_not) = tests_stack.pop().unwrap();
                    if tests.is_empty() {
                        return Err(token_info.expected("test name"));
                    }

                    is_not = prev_is_not;
                    match test_name {
                        Word::AllOf => Test::AllOf(tests),
                        Word::AnyOf => Test::AnyOf(tests),
                        _ => unreachable!(),
                    }
                }
                Token::Identifier(Word::Not) => {
                    is_not = !is_not;
                    continue;
                }
                Token::Identifier(word @ (Word::AnyOf | Word::AllOf)) => {
                    if tests_stack.len() < self.compiler.max_nested_tests {
                        self.expect_token(Token::ParenthesisOpen)?;
                        tests_stack.push((Vec::new(), word, is_not));
                        is_not = false;
                        continue;
                    } else {
                        return Err(CompileError {
                            line_num: token_info.line_num,
                            line_pos: token_info.line_pos,
                            error_type: ErrorType::TooManyNestedTests,
                        });
                    }
                }
                Token::Identifier(Word::True) => {
                    if !is_not {
                        Test::True
                    } else {
                        is_not = false;
                        Test::False
                    }
                }
                Token::Identifier(Word::False) => {
                    if !is_not {
                        Test::False
                    } else {
                        is_not = false;
                        Test::True
                    }
                }
                Token::Identifier(Word::Address) => self.parse_test_address()?,
                Token::Identifier(Word::Envelope) => self.parse_test_envelope()?,
                Token::Identifier(Word::Header) => self.parse_test_header()?,
                Token::Identifier(Word::Size) => self.parse_test_size()?,
                Token::Identifier(Word::Exists) => Test::Exists(self.parse_strings(false)?),
                _ => return Err(token_info.expected("test name")),
            };

            if is_not {
                test = Test::Not(test.into());
                is_not = false;
            }

            if let Some((tests, _, _)) = tests_stack.last_mut() {
                tests.push(test);
            } else {
                return Ok(test);
            }
        }
    }
}
