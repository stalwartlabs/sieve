use serde::{Deserialize, Serialize};

use crate::compiler::{
    lexer::{tokenizer::Tokenizer, word::Word, Token},
    CompileError, ErrorType,
};

use super::{
    action_convert::Convert,
    command::Command,
    test_address::TestAddress,
    test_body::TestBody,
    test_date::{TestCurrentDate, TestDate},
    test_duplicate::TestDuplicate,
    test_envelope::TestEnvelope,
    test_exists::TestExists,
    test_header::TestHeader,
    test_notify::{TestNotifyMethodCapability, TestValidNotifyMethod},
    test_size::TestSize,
    test_string::TestString,
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
    Exists(TestExists),
    Header(TestHeader),
    Size(TestSize),

    // RFC 5173
    Body(TestBody),

    // RFC 6558
    Convert(Convert),

    // RFC 5260
    Date(TestDate),
    CurrentDate(TestCurrentDate),

    // RFC 7352
    Duplicate(TestDuplicate),

    // RFC 5229
    String(TestString),

    // RFC 5435
    NotifyMethodCapability(TestNotifyMethodCapability),
    ValidNotifyMethod(TestValidNotifyMethod),
}

impl<'x> Tokenizer<'x> {
    pub(crate) fn parse_test(&mut self) -> Result<Test, CompileError> {
        let mut is_not = false;
        let mut p_count = 0;
        let mut tests_stack: Vec<(Vec<Test>, Word, bool, u32)> = Vec::new();

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
                Token::ParenthesisOpen => {
                    p_count += 1;
                    continue;
                }
                Token::ParenthesisClose => {
                    if p_count > 0 {
                        p_count -= 1;
                        continue;
                    } else if !tests_stack.is_empty() {
                        let (tests, test_name, prev_is_not, prev_p_count) =
                            tests_stack.pop().unwrap();
                        if tests.is_empty() {
                            return Err(token_info.expected("test name"));
                        }

                        is_not = prev_is_not;
                        p_count = prev_p_count;

                        match test_name {
                            Word::AllOf => Test::AllOf(tests),
                            Word::AnyOf => Test::AnyOf(tests),
                            _ => unreachable!(),
                        }
                    } else {
                        return Err(token_info.expected("test name"));
                    }
                }
                Token::Identifier(Word::Not) => {
                    is_not = !is_not;
                    continue;
                }
                Token::Identifier(word @ (Word::AnyOf | Word::AllOf)) => {
                    if tests_stack.len() < self.compiler.max_nested_tests {
                        self.expect_token(Token::ParenthesisOpen)?;
                        tests_stack.push((Vec::new(), word, is_not, p_count));
                        is_not = false;
                        p_count = 0;
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
                Token::Identifier(Word::Exists) => self.parse_test_exists()?,

                // RFC 5173
                Token::Identifier(Word::Body) => self.parse_test_body()?,

                // RFC 6558
                Token::Identifier(Word::Convert) => Test::Convert(self.parse_convert()?),

                // RFC 5260
                Token::Identifier(Word::Date) => self.parse_test_date()?,
                Token::Identifier(Word::CurrentDate) => self.parse_test_currentdate()?,

                // RFC 7352
                Token::Identifier(Word::Duplicate) => self.parse_test_duplicate()?,

                // RFC 5229
                Token::Identifier(Word::String) => self.parse_test_string()?,

                // RFC 5435
                Token::Identifier(Word::NotifyMethodCapability) => {
                    self.parse_test_notify_method_capability()?
                }
                Token::Identifier(Word::ValidNotifyMethod) => {
                    self.parse_test_valid_notify_method()?
                }

                _ => return Err(token_info.expected("test name")),
            };

            while p_count > 0 {
                self.expect_token(Token::ParenthesisClose)?;
                p_count -= 1;
            }

            if is_not {
                test = Test::Not(test.into());
                is_not = false;
            }

            if let Some((tests, _, _, _)) = tests_stack.last_mut() {
                tests.push(test);
            } else {
                return Ok(test);
            }
        }
    }
}
