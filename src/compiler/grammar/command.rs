use serde::{Deserialize, Serialize};

use crate::{
    compiler::{
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError, ErrorType,
    },
    Compiler, Sieve,
};

use super::{
    actions::{
        action_convert::Convert,
        action_editheader::{AddHeader, DeleteHeader},
        action_fileinto::FileInto,
        action_flags::FlagAction,
        action_include::{Global, Include},
        action_keep::Keep,
        action_mime::{Break, Enclose, ExtractText, ForEveryPart, Replace},
        action_notify::Notify,
        action_redirect::Redirect,
        action_reject::Reject,
        action_set::Set,
        action_vacation::Vacation,
    },
    test::{If, Test},
};

use super::tests::test_ihave::Error;

#[derive(Debug, Serialize, Deserialize)]
pub(crate) enum Command {
    If(Vec<If>),
    Keep(Keep),
    FileInto(FileInto),
    Redirect(Redirect),
    Discard,
    Stop,
    Invalid(String),

    // RFC 5703
    ForEveryPart(ForEveryPart),
    Break(Break),
    Replace(Replace),
    Enclose(Enclose),
    ExtractText(ExtractText),

    // RFC 6558
    Convert(Convert),

    // RFC 5293
    AddHeader(AddHeader),
    DeleteHeader(DeleteHeader),

    // RFC 5229
    Set(Set),

    // RFC 5435
    Notify(Notify),

    // RFC 5429
    Reject(Reject),

    // RFC 5230
    Vacation(Vacation),

    // RFC 5463
    Error(Error),

    // RFC 5232
    SetFlag(FlagAction),
    AddFlag(FlagAction),
    RemoveFlag(FlagAction),

    // RFC 6609
    Include(Include),
    Return,
    Global(Global),
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
                            commands.push(Command::Keep(tokens.parse_keep()?));
                        }
                        Word::FileInto => {
                            commands.push(Command::FileInto(tokens.parse_fileinto()?));
                        }
                        Word::Redirect => {
                            commands.push(Command::Redirect(tokens.parse_redirect()?));
                        }
                        Word::Discard => {
                            commands.push(Command::Discard);
                        }
                        Word::Stop => {
                            commands.push(Command::Stop);
                        }

                        // RFC 5703
                        Word::ForEveryPart => {
                            commands.push(Command::ForEveryPart(ForEveryPart {
                                name: if let Some(Ok(Token::Tag(Word::Name))) =
                                    tokens.peek().map(|r| r.map(|t| &t.token))
                                {
                                    tokens.next();
                                    tokens.unwrap_static_string()?.into()
                                } else {
                                    None
                                },
                                commands: Vec::new(),
                            }));
                            is_new_block = true;
                        }
                        Word::Break => {
                            commands.push(Command::Break(Break {
                                name: if let Some(Ok(Token::Tag(Word::Name))) =
                                    tokens.peek().map(|r| r.map(|t| &t.token))
                                {
                                    tokens.next();
                                    tokens.unwrap_static_string()?.into()
                                } else {
                                    None
                                },
                            }));
                        }
                        Word::Replace => {
                            commands.push(Command::Replace(tokens.parse_replace()?));
                        }
                        Word::Enclose => {
                            commands.push(Command::Enclose(tokens.parse_enclose()?));
                        }
                        Word::ExtractText => {
                            commands.push(Command::ExtractText(tokens.parse_extracttext()?));
                        }

                        // RFC 6558
                        Word::Convert => {
                            commands.push(Command::Convert(tokens.parse_convert()?));
                        }

                        // RFC 5293
                        Word::AddHeader => {
                            commands.push(Command::AddHeader(tokens.parse_addheader()?));
                        }
                        Word::DeleteHeader => {
                            commands.push(Command::DeleteHeader(tokens.parse_deleteheader()?));
                        }

                        // RFC 5229
                        Word::Set => {
                            commands.push(Command::Set(tokens.parse_set()?));
                        }

                        // RFC 5435
                        Word::Notify => {
                            commands.push(Command::Notify(tokens.parse_notify()?));
                        }

                        // RFC 5429
                        Word::Reject => {
                            commands.push(Command::Reject(tokens.parse_reject(false)?));
                        }
                        Word::Ereject => {
                            commands.push(Command::Reject(tokens.parse_reject(true)?));
                        }

                        // RFC 5230
                        Word::Vacation => {
                            commands.push(Command::Vacation(tokens.parse_vacation()?));
                        }

                        // RFC 5463
                        Word::Error => {
                            commands.push(Command::Error(tokens.parse_error()?));
                        }

                        // RFC 5232
                        Word::SetFlag => {
                            commands.push(Command::SetFlag(tokens.parse_flag_action()?));
                        }
                        Word::AddFlag => {
                            commands.push(Command::AddFlag(tokens.parse_flag_action()?));
                        }
                        Word::RemoveFlag => {
                            commands.push(Command::RemoveFlag(tokens.parse_flag_action()?));
                        }

                        // RFC 6609
                        Word::Include => {
                            commands.push(Command::Include(tokens.parse_include()?));
                        }
                        Word::Return => {
                            commands.push(Command::Return);
                        }
                        Word::Global => {
                            commands.push(Command::Global(tokens.parse_global()?));
                        }

                        _ => {
                            #[cfg(test)]
                            {
                                tokens.ignore_command()?;
                                commands.push(Command::Invalid(command.to_string()));
                                continue;
                            }
                            #[cfg(not(test))]
                            {
                                return Err(token_info.expected("command"));
                            }
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
                        Some(Command::ForEveryPart(fep)) => {
                            fep.commands = commands;
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
