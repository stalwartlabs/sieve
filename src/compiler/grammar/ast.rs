use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError, ErrorType,
    },
    runtime::Command,
    Compiler, Sieve,
};

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
        let mut last_command = Word::Not;

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
                            is_new_block = true;
                        }
                        Word::ElsIf if matches!(last_command, Word::If | Word::ElsIf) => {
                            is_new_block = true;
                        }
                        Word::Else if matches!(last_command, Word::If | Word::ElsIf) => {
                            tokens.expect_token(Token::CurlyOpen)?;
                            is_new_block = true;
                        }
                        Word::Keep => {
                            commands.push(Command::Keep);
                            tokens.expect_command_end()?;
                        }
                        Word::FileInto => {
                            commands.push(Command::FileInto {
                                mailbox: tokens.unwrap_string()?,
                            });
                            tokens.expect_command_end()?;
                        }
                        Word::Redirect => {
                            commands.push(Command::Redirect {
                                address: tokens.unwrap_string()?,
                            });
                            tokens.expect_command_end()?;
                        }
                        Word::Discard => {
                            commands.push(Command::Discard);
                            tokens.expect_command_end()?;
                        }
                        Word::Stop => {
                            commands.push(Command::Stop);
                            tokens.expect_command_end()?;
                        }
                        _ => {
                            return Err(token_info.into());
                        }
                    }

                    if is_new_block {
                        block_line_num = tokens.line_num;
                        block_line_pos = tokens.pos - tokens.line_start;

                        if block_stack.len() < self.max_nested_blocks {
                            block_stack.push((commands, command));
                            commands = Vec::new();
                            last_command = Word::Not;
                        } else {
                            return Err(CompileError {
                                line_num: block_line_num,
                                line_pos: block_line_pos,
                                error_type: ErrorType::TooManyNestedBlocks,
                            });
                        }
                    } else {
                        last_command = command;
                    }
                }
                Token::CurlyClose if !block_stack.is_empty() => {
                    let (prev_commands, prev_command) = block_stack.pop().unwrap();

                    commands = prev_commands;
                    last_command = prev_command;
                }
                Token::Invalid(command) => {
                    tokens.ignore_command()?;
                    commands.push(Command::Invalid { command });
                    last_command = Word::Not;
                }
                _ => {
                    return Err(token_info.into());
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
