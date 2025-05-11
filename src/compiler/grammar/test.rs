/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    lexer::{tokenizer::TokenInfo, word::Word, Token},
    CompileError, ErrorType,
};

use super::{
    actions::{action_convert::Convert, action_vacation::TestVacation},
    expr::{parser::ExpressionParser, tokenizer::Tokenizer, Expression, UnaryOperator},
    instruction::{CompilerState, Instruction},
    tests::{
        test_address::TestAddress,
        test_body::TestBody,
        test_date::{TestCurrentDate, TestDate},
        test_duplicate::TestDuplicate,
        test_envelope::TestEnvelope,
        test_exists::TestExists,
        test_extlists::TestValidExtList,
        test_hasflag::TestHasFlag,
        test_header::TestHeader,
        test_ihave::TestIhave,
        test_mailbox::{TestMailboxExists, TestMetadata, TestMetadataExists},
        test_mailboxid::TestMailboxIdExists,
        test_notify::{TestNotifyMethodCapability, TestValidNotifyMethod},
        test_size::TestSize,
        test_spamtest::{TestSpamTest, TestVirusTest},
        test_specialuse::TestSpecialUseExists,
        test_string::TestString,
    },
    Capability, Invalid,
};

#[allow(clippy::enum_variant_names)]
#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum Test {
    True,
    False,
    Address(TestAddress),
    Envelope(TestEnvelope),
    Exists(TestExists),
    Header(TestHeader),
    Size(TestSize),
    Invalid(Invalid),

    // RFC 5173
    Body(TestBody),

    // RFC 6558
    Convert(Convert),

    // RFC 5260
    Date(TestDate),
    CurrentDate(TestCurrentDate),

    // RFC 7352
    Duplicate(TestDuplicate),

    // RFC 5229 & RFC 5183
    String(TestString),
    Environment(TestString),

    // RFC 5435
    NotifyMethodCapability(TestNotifyMethodCapability),
    ValidNotifyMethod(TestValidNotifyMethod),

    // RFC 6134
    ValidExtList(TestValidExtList),

    // RFC 5463
    Ihave(TestIhave),

    // RFC 5232
    HasFlag(TestHasFlag),

    // RFC 5490
    MailboxExists(TestMailboxExists),
    Metadata(TestMetadata),
    MetadataExists(TestMetadataExists),

    // RFC 9042
    MailboxIdExists(TestMailboxIdExists),

    // RFC 5235
    SpamTest(TestSpamTest),
    VirusTest(TestVirusTest),

    // RFC 8579
    SpecialUseExists(TestSpecialUseExists),

    // RFC 5230
    Vacation(TestVacation),

    // Only test
    #[cfg(test)]
    TestCmd {
        arguments: Vec<crate::compiler::Value>,
        is_not: bool,
    },
}

#[derive(Debug)]
struct Block {
    is_all: bool,
    is_not: bool,
    p_count: u32,
    jmps: Vec<usize>,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test(&mut self) -> Result<(), CompileError> {
        let mut block_stack: Vec<Block> = Vec::new();
        let mut block = Block {
            is_all: false,
            is_not: false,
            p_count: 0,
            jmps: Vec::new(),
        };
        let mut is_not = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            self.reset_param_check();
            let test: Instruction =
                match token_info.token {
                    Token::Comma
                        if !block_stack.is_empty()
                            && matches!(
                                self.instructions.last(),
                                Some(Instruction::Test(_) | Instruction::Eval(_))
                            )
                            && matches!(
                                self.tokens.peek(),
                                Some(Ok(TokenInfo {
                                    token: Token::Identifier(_) | Token::Unknown(_),
                                    ..
                                }))
                            ) =>
                    {
                        is_not = block.is_not;
                        block.jmps.push(self.instructions.len());
                        self.instructions.push(if block.is_all {
                            Instruction::Jz(usize::MAX)
                        } else {
                            Instruction::Jnz(usize::MAX)
                        });
                        continue;
                    }
                    Token::ParenthesisOpen => {
                        block.p_count += 1;
                        continue;
                    }
                    Token::ParenthesisClose => {
                        if block.p_count > 0 {
                            block.p_count -= 1;
                            continue;
                        } else if let Some(prev_block) = block_stack.pop() {
                            let cur_pos = self.instructions.len();
                            for jmp_pos in block.jmps {
                                if let Instruction::Jnz(jmp_pos) | Instruction::Jz(jmp_pos) =
                                    &mut self.instructions[jmp_pos]
                                {
                                    *jmp_pos = cur_pos;
                                } else {
                                    debug_assert!(false, "This should not have happened")
                                }
                            }

                            block = prev_block;
                            is_not = block.is_not;
                            if block_stack.is_empty() {
                                break;
                            } else {
                                continue;
                            }
                        } else {
                            return Err(token_info.expected("test name"));
                        }
                    }
                    Token::Identifier(Word::Not) => {
                        if !matches!(
                            self.tokens.peek(),
                            Some(Ok(TokenInfo {
                                token: Token::Identifier(_) | Token::Unknown(_),
                                ..
                            }))
                        ) {
                            return Err(token_info.expected("test name"));
                        }
                        is_not = !is_not;
                        continue;
                    }
                    Token::Identifier(word @ (Word::AnyOf | Word::AllOf)) => {
                        if block_stack.len() < self.tokens.compiler.max_nested_tests {
                            self.tokens.expect_token(Token::ParenthesisOpen)?;
                            block_stack.push(block);
                            let (is_all, block_is_not) = if word == Word::AllOf {
                                if !is_not {
                                    (true, false)
                                } else {
                                    (false, true)
                                }
                            } else if !is_not {
                                (false, false)
                            } else {
                                (true, true)
                            };
                            block = Block {
                                is_all,
                                is_not: block_is_not,
                                p_count: 0,
                                jmps: Vec::new(),
                            };
                            is_not = block_is_not;
                            continue;
                        } else {
                            return Err(CompileError {
                                line_num: token_info.line_num,
                                line_pos: token_info.line_pos,
                                error_type: ErrorType::TooManyNestedTests,
                            });
                        }
                    }
                    Token::Identifier(Word::True) => if !is_not {
                        Test::True
                    } else {
                        is_not = false;
                        Test::False
                    }
                    .into(),
                    Token::Identifier(Word::False) => if !is_not {
                        Test::False
                    } else {
                        is_not = false;
                        Test::True
                    }
                    .into(),
                    Token::Identifier(Word::Address) => self.parse_test_address()?.into(),
                    Token::Identifier(Word::Envelope) => {
                        self.validate_argument(
                            0,
                            Capability::Envelope.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_envelope()?.into()
                    }
                    Token::Identifier(Word::Header) => self.parse_test_header()?.into(),
                    Token::Identifier(Word::Size) => self.parse_test_size()?.into(),
                    Token::Identifier(Word::Exists) => self.parse_test_exists()?.into(),

                    // RFC 5173
                    Token::Identifier(Word::Body) => {
                        self.validate_argument(
                            0,
                            Capability::Body.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_body()?.into()
                    }

                    // RFC 6558
                    Token::Identifier(Word::Convert) => {
                        self.validate_argument(
                            0,
                            Capability::Convert.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_convert()?.into()
                    }

                    // RFC 5260
                    Token::Identifier(Word::Date) => {
                        self.validate_argument(
                            0,
                            Capability::Date.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_date()?.into()
                    }
                    Token::Identifier(Word::CurrentDate) => {
                        self.validate_argument(
                            0,
                            Capability::Date.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_currentdate()?.into()
                    }

                    // RFC 7352
                    Token::Identifier(Word::Duplicate) => {
                        self.validate_argument(
                            0,
                            Capability::Duplicate.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_duplicate()?.into()
                    }

                    // RFC 5229
                    Token::Identifier(Word::String) => {
                        self.validate_argument(
                            0,
                            Capability::Variables.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_string()?.into()
                    }

                    // RFC 5435
                    Token::Identifier(Word::NotifyMethodCapability) => {
                        self.validate_argument(
                            0,
                            Capability::Enotify.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_notify_method_capability()?.into()
                    }
                    Token::Identifier(Word::ValidNotifyMethod) => {
                        self.validate_argument(
                            0,
                            Capability::Enotify.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_valid_notify_method()?.into()
                    }

                    // RFC 5183
                    Token::Identifier(Word::Environment) => {
                        self.validate_argument(
                            0,
                            Capability::Environment.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_environment()?.into()
                    }

                    // RFC 6134
                    Token::Identifier(Word::ValidExtList) => {
                        self.validate_argument(
                            0,
                            Capability::ExtLists.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_valid_ext_list()?.into()
                    }

                    // RFC 5463
                    Token::Identifier(Word::Ihave) => {
                        self.validate_argument(
                            0,
                            Capability::Ihave.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_ihave()?.into()
                    }

                    // RFC 5232
                    Token::Identifier(Word::HasFlag) => {
                        self.validate_argument(
                            0,
                            Capability::Imap4Flags.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_hasflag()?.into()
                    }

                    // RFC 5490
                    Token::Identifier(Word::MailboxExists) => {
                        self.validate_argument(
                            0,
                            Capability::Mailbox.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_mailboxexists()?.into()
                    }
                    Token::Identifier(Word::Metadata) => {
                        self.validate_argument(
                            0,
                            Capability::MboxMetadata.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_metadata()?.into()
                    }
                    Token::Identifier(Word::MetadataExists) => {
                        self.validate_argument(
                            0,
                            Capability::MboxMetadata.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_metadataexists()?.into()
                    }
                    Token::Identifier(Word::ServerMetadata) => {
                        self.validate_argument(
                            0,
                            Capability::ServerMetadata.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_servermetadata()?.into()
                    }
                    Token::Identifier(Word::ServerMetadataExists) => {
                        self.validate_argument(
                            0,
                            Capability::ServerMetadata.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_servermetadataexists()?.into()
                    }

                    // RFC 9042
                    Token::Identifier(Word::MailboxIdExists) => {
                        self.validate_argument(
                            0,
                            Capability::MailboxId.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_mailboxidexists()?.into()
                    }

                    // RFC 5235
                    Token::Identifier(Word::SpamTest) => {
                        self.validate_argument(
                            0,
                            Capability::SpamTest.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_spamtest()?.into()
                    }
                    Token::Identifier(Word::VirusTest) => {
                        self.validate_argument(
                            0,
                            Capability::VirusTest.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_virustest()?.into()
                    }

                    // RFC 8579
                    Token::Identifier(Word::SpecialUseExists) => {
                        self.validate_argument(
                            0,
                            Capability::SpecialUse.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;
                        self.parse_test_specialuseexists()?.into()
                    }

                    // Expressions extension
                    Token::Identifier(Word::Eval) => {
                        self.validate_argument(
                            0,
                            Capability::Expressions.into(),
                            token_info.line_num,
                            token_info.line_pos,
                        )?;

                        Instruction::Eval(self.parse_expr()?)
                    }
                    Token::Identifier(word) => {
                        self.ignore_test()?;
                        Test::Invalid(Invalid {
                            name: word.to_string(),
                            line_num: token_info.line_num,
                            line_pos: token_info.line_pos,
                        })
                        .into()
                    }
                    #[cfg(test)]
                    Token::Unknown(name) if name.contains("test") => {
                        use crate::compiler::Value;

                        let mut arguments = Vec::new();
                        arguments.push(Value::Text(name.into()));
                        while !matches!(
                            self.tokens.peek().map(|r| r.map(|t| &t.token)),
                            Some(Ok(Token::Comma
                                | Token::ParenthesisClose
                                | Token::CurlyOpen))
                        ) {
                            arguments.push(match self.tokens.unwrap_next()?.token {
                                Token::StringConstant(s) => Value::from(s),
                                Token::StringVariable(s) => self
                                    .tokenize_string(&s, true)
                                    .map_err(|error_type| CompileError {
                                        line_num: 0,
                                        line_pos: 0,
                                        error_type,
                                    })?,
                                Token::Number(n) => {
                                    Value::Number(crate::compiler::Number::Integer(n as i64))
                                }
                                Token::Identifier(s) => Value::Text(s.to_string().into()),
                                Token::Tag(s) => Value::Text(format!(":{s}").into()),
                                Token::Unknown(s) => Value::Text(s.into()),
                                other => panic!("Invalid test param {other:?}"),
                            });
                        }
                        Test::TestCmd {
                            arguments,
                            is_not: false,
                        }
                        .into()
                    }
                    Token::Unknown(name) => {
                        self.ignore_test()?;
                        Test::Invalid(Invalid {
                            name,
                            line_num: token_info.line_num,
                            line_pos: token_info.line_pos,
                        })
                        .into()
                    }
                    _ => return Err(token_info.expected("test name")),
                };

            while block.p_count > 0 {
                self.tokens.expect_token(Token::ParenthesisClose)?;
                block.p_count -= 1;
            }

            self.instructions
                .push(if !is_not { test } else { test.set_not() });

            if block_stack.is_empty() {
                break;
            }
        }

        self.instructions.push(Instruction::Jz(usize::MAX));
        Ok(())
    }

    pub(crate) fn parse_expr(&mut self) -> Result<Vec<Expression>, CompileError> {
        let mut next_token = self.tokens.unwrap_next()?;
        let expr = match next_token.token {
            Token::StringConstant(s) => s.into_string().into_bytes(),
            Token::StringVariable(s) => s,
            _ => return Err(next_token.expected("string")),
        };

        match ExpressionParser::from_tokenizer(Tokenizer::from_iter(
            expr.iter().enumerate().peekable(),
            |var_name, maybe_namespace| self.parse_expr_fnc_or_var(var_name, maybe_namespace),
        ))
        .parse()
        {
            Ok(parser) => Ok(parser.output),
            Err(err) => {
                let err = ErrorType::InvalidExpression(format!(
                    "{}: {}",
                    std::str::from_utf8(&expr).unwrap_or_default(),
                    err
                ));
                next_token.token = Token::StringVariable(expr);
                Err(next_token.custom(err))
            }
        }
    }
}

impl From<Test> for Instruction {
    fn from(test: Test) -> Self {
        Instruction::Test(test)
    }
}

impl Instruction {
    pub fn set_not(mut self) -> Self {
        match &mut self {
            Instruction::Test(test) => match test {
                Test::True => return Instruction::Test(Test::False),
                Test::False => return Instruction::Test(Test::True),
                Test::Address(op) => {
                    op.is_not = true;
                }
                Test::Envelope(op) => {
                    op.is_not = true;
                }
                Test::Exists(op) => {
                    op.is_not = true;
                }
                Test::Header(op) => {
                    op.is_not = true;
                }
                Test::Size(op) => {
                    op.is_not = true;
                }
                Test::Body(op) => {
                    op.is_not = true;
                }
                Test::Convert(op) => {
                    op.is_not = true;
                }
                Test::Date(op) => {
                    op.is_not = true;
                }
                Test::CurrentDate(op) => {
                    op.is_not = true;
                }
                Test::Duplicate(op) => {
                    op.is_not = true;
                }
                Test::String(op) | Test::Environment(op) => {
                    op.is_not = true;
                }
                Test::NotifyMethodCapability(op) => {
                    op.is_not = true;
                }
                Test::ValidNotifyMethod(op) => {
                    op.is_not = true;
                }
                Test::ValidExtList(op) => {
                    op.is_not = true;
                }
                Test::Ihave(op) => {
                    op.is_not = true;
                }
                Test::HasFlag(op) => {
                    op.is_not = true;
                }
                Test::MailboxExists(op) => {
                    op.is_not = true;
                }
                Test::Metadata(op) => {
                    op.is_not = true;
                }
                Test::MetadataExists(op) => {
                    op.is_not = true;
                }
                Test::MailboxIdExists(op) => {
                    op.is_not = true;
                }
                Test::SpamTest(op) => {
                    op.is_not = true;
                }
                Test::VirusTest(op) => {
                    op.is_not = true;
                }
                Test::SpecialUseExists(op) => {
                    op.is_not = true;
                }
                #[cfg(test)]
                Test::TestCmd { is_not, .. } => {
                    *is_not = true;
                }
                Test::Vacation(_) | Test::Invalid(_) => {}
            },
            Instruction::Eval(expr) => expr.push(Expression::UnaryOperator(UnaryOperator::Not)),
            _ => (),
        }
        self
    }
}
