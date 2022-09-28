use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError, ErrorType,
    },
    runtime::StringItem,
    Compiler, Sieve,
};

use super::test::{If, Test};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Command {
    If(Vec<If>),
    Keep,
    FileInto(StringItem),
    Redirect(StringItem),
    Discard,
    Stop,
    Invalid(String),
}

impl Compiler {
    pub fn compile(&self, script: &[u8]) -> Result<Sieve, CompileError> {
        if script.len() > self.max_script_len {
            return Err(CompileError {
                line_num: 0,
                line_pos: 0,
                error_type: ErrorType::ScriptTooLong,
            });
        }

        let mut tokens = Tokenizer::new(self, script);
        let mut commands = Vec::new();
        let mut capabilities = Vec::new();

        let mut block_stack = Vec::new();
        let mut block_line_num = 0;
        let mut block_line_pos = 0;

        while let Some(token_info) = tokens.next() {
            let token_info = token_info?;

            match token_info.token {
                Token::Identifier(command) => {
                    let mut is_new_block = false;

                    match command {
                        Word::Require => {
                            tokens.parse_require(&mut capabilities)?;
                        }
                        Word::If => {
                            commands.push(Command::If(vec![If {
                                test: tokens.parse_test()?,
                                commands: Vec::new(),
                            }]));
                            is_new_block = true;
                        }
                        Word::ElsIf => {
                            if let Some(Command::If(ifs)) = commands.last_mut() {
                                if ifs.last().unwrap().test != Test::True {
                                    ifs.push(If {
                                        test: tokens.parse_test()?,
                                        commands: Vec::new(),
                                    });
                                    is_new_block = true;
                                }
                            }
                            if !is_new_block {
                                return Err(token_info.expected("'if' before 'elsif'"));
                            }
                        }
                        Word::Else => {
                            if let Some(Command::If(ifs)) = commands.last_mut() {
                                if ifs.last().unwrap().test != Test::True {
                                    ifs.push(If {
                                        test: Test::True,
                                        commands: Vec::new(),
                                    });
                                    is_new_block = true;
                                }
                            }
                            if !is_new_block {
                                return Err(token_info.expected("'if' or 'elsif' before 'else'"));
                            }
                        }
                        Word::Keep => {
                            commands.push(Command::Keep);
                        }
                        Word::FileInto => {
                            commands.push(Command::FileInto(tokens.unwrap_string()?));
                        }
                        Word::Redirect => {
                            commands.push(Command::Redirect(tokens.unwrap_string()?));
                        }
                        Word::Discard => {
                            commands.push(Command::Discard);
                        }
                        Word::Stop => {
                            commands.push(Command::Stop);
                        }
                        _ => {
                            return Err(token_info.expected("command"));
                        }
                    }

                    if is_new_block {
                        block_line_num = tokens.line_num;
                        block_line_pos = tokens.pos - tokens.line_start;

                        tokens.expect_token(Token::CurlyOpen)?;
                        if block_stack.len() < self.max_nested_blocks {
                            block_stack.push(commands);
                            commands = Vec::new();
                        } else {
                            return Err(CompileError {
                                line_num: block_line_num,
                                line_pos: block_line_pos,
                                error_type: ErrorType::TooManyNestedBlocks,
                            });
                        }
                    } else {
                        tokens.expect_command_end()?;
                    }
                }
                Token::CurlyClose if !block_stack.is_empty() => {
                    let mut prev_commands = block_stack.pop().unwrap();
                    match prev_commands.last_mut() {
                        Some(Command::If(ifs)) => {
                            ifs.last_mut().unwrap().commands = commands;
                        }
                        _ => debug_assert!(false, "This should not have happened."),
                    }

                    commands = prev_commands;
                }
                Token::Invalid(command) => {
                    tokens.ignore_command()?;
                    commands.push(Command::Invalid(command));
                }
                _ => {
                    return Err(token_info.expected("command"));
                }
            }
        }

        if block_stack.is_empty() {
            Ok(Sieve {
                capabilities,
                commands,
            })
        } else {
            Err(CompileError {
                line_num: block_line_num,
                line_pos: block_line_pos,
                error_type: ErrorType::UnterminatedBlock,
            })
        }
    }
}
