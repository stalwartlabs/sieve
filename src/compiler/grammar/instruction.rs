/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use ahash::{AHashMap, AHashSet};

use crate::{
    compiler::{
        grammar::{test::Test, MatchType},
        lexer::{tokenizer::Tokenizer, word::Word, Token},
        CompileError, ErrorType, Value, VariableType,
    },
    Compiler, Sieve,
};

use super::{
    actions::{
        action_convert::Convert,
        action_editheader::{AddHeader, DeleteHeader},
        action_fileinto::FileInto,
        action_flags::EditFlags,
        action_include::Include,
        action_keep::Keep,
        action_mime::{Enclose, ExtractText, ForEveryPart, Replace},
        action_notify::Notify,
        action_redirect::Redirect,
        action_reject::Reject,
        action_set::{Let, Set},
        action_vacation::Vacation,
    },
    expr::Expression,
    Capability, Clear, Invalid, While,
};

use super::tests::test_ihave::Error;

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum Instruction {
    Require(Vec<Capability>),
    Keep(Keep),
    FileInto(FileInto),
    Redirect(Redirect),
    Discard,
    Stop,
    Invalid(Invalid),
    Test(Test),
    Jmp(usize),
    Jz(usize),
    Jnz(usize),

    // RFC 5703
    ForEveryPartPush,
    ForEveryPart(ForEveryPart),
    ForEveryPartPop(usize),
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
    Clear(Clear),

    // RFC 5435
    Notify(Notify),

    // RFC 5429
    Reject(Reject),

    // RFC 5230
    Vacation(Vacation),

    // RFC 5463
    Error(Error),

    // RFC 5232
    EditFlags(EditFlags),

    // RFC 6609
    Include(Include),
    Return,

    // For every line extension
    While(While),

    // Expression extension
    Eval(Vec<Expression>),
    Let(Let),

    // Test only
    #[cfg(test)]
    TestCmd(Vec<Value>),
}

pub(crate) const MAX_PARAMS: usize = 11;

#[derive(Debug)]
pub(crate) struct Block {
    pub(crate) btype: Word,
    pub(crate) label: Option<String>,
    pub(crate) line_num: usize,
    pub(crate) line_pos: usize,
    pub(crate) last_block_start: usize,
    pub(crate) if_jmps: Vec<usize>,
    pub(crate) break_jmps: Vec<usize>,
    pub(crate) match_test_pos: Vec<usize>,
    pub(crate) match_test_vars: u64,
    pub(crate) vars_local: AHashMap<String, usize>,
    pub(crate) capabilities: AHashSet<Capability>,
    pub(crate) require_pos: usize,
}

pub(crate) struct CompilerState<'x> {
    pub(crate) compiler: &'x Compiler,
    pub(crate) tokens: Tokenizer<'x>,
    pub(crate) instructions: Vec<Instruction>,
    pub(crate) block_stack: Vec<Block>,
    pub(crate) block: Block,
    pub(crate) last_block_type: Word,
    pub(crate) vars_global: AHashSet<String>,
    pub(crate) vars_num: usize,
    pub(crate) vars_num_max: usize,
    pub(crate) vars_match_max: usize,
    pub(crate) vars_local: usize,
    pub(crate) param_check: [bool; MAX_PARAMS],
    pub(crate) includes_num: usize,
}

impl Compiler {
    pub fn compile(&self, script: &[u8]) -> Result<Sieve, CompileError> {
        if script.len() > self.max_script_size {
            return Err(CompileError {
                line_num: 0,
                line_pos: 0,
                error_type: ErrorType::ScriptTooLong,
            });
        }

        let mut state = CompilerState {
            compiler: self,
            tokens: Tokenizer::new(self, script),
            instructions: Vec::new(),
            block_stack: Vec::new(),
            block: Block::new(Word::Not),
            last_block_type: Word::Not,
            vars_global: AHashSet::new(),
            vars_num: 0,
            vars_num_max: 0,
            vars_match_max: 0,
            vars_local: 0,
            param_check: [false; MAX_PARAMS],
            includes_num: 0,
        };

        while let Some(token_info) = state.tokens.next() {
            let token_info = token_info?;
            state.reset_param_check();

            match token_info.token {
                Token::Identifier(instruction) => {
                    let mut is_new_block = None;

                    match instruction {
                        Word::Require => {
                            state.parse_require()?;
                        }
                        Word::If => {
                            state.parse_test()?;
                            state.block.if_jmps.clear();
                            is_new_block = Block::new(Word::If).into();
                        }
                        Word::ElsIf => {
                            if let Word::If | Word::ElsIf = &state.last_block_type {
                                state.parse_test()?;
                                is_new_block = Block::new(Word::ElsIf).into();
                            } else {
                                return Err(token_info.expected("'if' before 'elsif'"));
                            }
                        }
                        Word::Else => {
                            if let Word::If | Word::ElsIf = &state.last_block_type {
                                is_new_block = Block::new(Word::Else).into();
                            } else {
                                return Err(token_info.expected("'if' or 'elsif' before 'else'"));
                            }
                        }
                        Word::Keep => {
                            state.parse_keep()?;
                        }
                        Word::FileInto => {
                            state.validate_argument(
                                0,
                                Capability::FileInto.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_fileinto()?;
                        }
                        Word::Redirect => {
                            state.parse_redirect()?;
                        }
                        Word::Discard => {
                            state.instructions.push(Instruction::Discard);
                        }
                        Word::Stop => {
                            state.instructions.push(Instruction::Stop);
                        }

                        // RFC 5703
                        Word::ForEveryPart => {
                            state.validate_argument(
                                0,
                                Capability::ForEveryPart.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;

                            if state
                                .block_stack
                                .iter()
                                .filter(|b| matches!(&b.btype, Word::ForEveryPart))
                                .count()
                                == self.max_nested_foreverypart
                            {
                                return Err(
                                    token_info.custom(ErrorType::TooManyNestedForEveryParts)
                                );
                            }

                            is_new_block = if let Some(Ok(Token::Tag(Word::Name))) =
                                state.tokens.peek().map(|r| r.map(|t| &t.token))
                            {
                                let tag = state.tokens.next().unwrap().unwrap();
                                let label = state.tokens.expect_static_string()?;
                                for block in &state.block_stack {
                                    if block.label.as_ref().is_some_and(|n| n.eq(&label)) {
                                        return Err(
                                            tag.custom(ErrorType::LabelAlreadyDefined(label))
                                        );
                                    }
                                }
                                Block::new(Word::ForEveryPart).with_label(label)
                            } else {
                                Block::new(Word::ForEveryPart)
                            }
                            .into();

                            state.instructions.push(Instruction::ForEveryPartPush);
                            state
                                .instructions
                                .push(Instruction::ForEveryPart(ForEveryPart {
                                    jz_pos: usize::MAX,
                                }));
                        }
                        Word::Break => {
                            if let Some(Ok(Token::Tag(Word::Name))) =
                                state.tokens.peek().map(|r| r.map(|t| &t.token))
                            {
                                state.validate_argument(
                                    0,
                                    Capability::ForEveryPart.into(),
                                    token_info.line_num,
                                    token_info.line_pos,
                                )?;

                                let tag = state.tokens.next().unwrap().unwrap();
                                let label = state.tokens.expect_static_string()?;
                                let mut label_found = false;
                                let mut num_pops = 0;

                                for block in [&mut state.block]
                                    .into_iter()
                                    .chain(state.block_stack.iter_mut().rev())
                                {
                                    if let Word::ForEveryPart = &block.btype {
                                        num_pops += 1;
                                        if block.label.as_ref().is_some_and(|n| n.eq(&label)) {
                                            state
                                                .instructions
                                                .push(Instruction::ForEveryPartPop(num_pops));
                                            block.break_jmps.push(state.instructions.len());
                                            label_found = true;
                                            break;
                                        }
                                    }
                                }

                                if !label_found {
                                    return Err(tag.custom(ErrorType::LabelUndefined(label)));
                                }
                            } else {
                                let mut block_found = None;
                                if matches!(&state.block.btype, Word::ForEveryPart | Word::While) {
                                    block_found = Some(&mut state.block);
                                } else {
                                    for block in state.block_stack.iter_mut().rev() {
                                        if matches!(&block.btype, Word::ForEveryPart | Word::While)
                                        {
                                            block_found = Some(block);
                                            break;
                                        }
                                    }
                                }

                                let block = block_found.ok_or_else(|| {
                                    token_info.custom(ErrorType::BreakOutsideLoop)
                                })?;
                                if matches!(block.btype, Word::ForEveryPart) {
                                    state.instructions.push(Instruction::ForEveryPartPop(1));
                                }

                                block.break_jmps.push(state.instructions.len());
                            }

                            state.instructions.push(Instruction::Jmp(usize::MAX));
                        }
                        Word::Replace => {
                            state.validate_argument(
                                0,
                                Capability::Replace.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_replace()?;
                        }
                        Word::Enclose => {
                            state.validate_argument(
                                0,
                                Capability::Enclose.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_enclose()?;
                        }
                        Word::ExtractText => {
                            state.validate_argument(
                                0,
                                Capability::ExtractText.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_extracttext()?;
                        }

                        // RFC 6558
                        Word::Convert => {
                            state.validate_argument(
                                0,
                                Capability::Convert.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_convert()?;
                        }

                        // RFC 5293
                        Word::AddHeader => {
                            state.validate_argument(
                                0,
                                Capability::EditHeader.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_addheader()?;
                        }
                        Word::DeleteHeader => {
                            state.validate_argument(
                                0,
                                Capability::EditHeader.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_deleteheader()?;
                        }

                        // RFC 5229
                        Word::Set => {
                            state.validate_argument(
                                0,
                                Capability::Variables.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_set()?;
                        }

                        // RFC 5435
                        Word::Notify => {
                            state.validate_argument(
                                0,
                                Capability::Enotify.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_notify()?;
                        }

                        // RFC 5429
                        Word::Reject => {
                            state.validate_argument(
                                0,
                                Capability::Reject.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_reject(false)?;
                        }
                        Word::Ereject => {
                            state.validate_argument(
                                0,
                                Capability::Ereject.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_reject(true)?;
                        }

                        // RFC 5230
                        Word::Vacation => {
                            state.validate_argument(
                                0,
                                Capability::Vacation.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_vacation()?;
                        }

                        // RFC 5463
                        Word::Error => {
                            state.validate_argument(
                                0,
                                Capability::Ihave.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_error()?;
                        }

                        // RFC 5232
                        Word::SetFlag | Word::AddFlag | Word::RemoveFlag => {
                            state.validate_argument(
                                0,
                                Capability::Imap4Flags.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_flag_action(instruction)?;
                        }

                        // RFC 6609
                        Word::Include => {
                            if state.includes_num < self.max_includes {
                                state.validate_argument(
                                    0,
                                    Capability::Include.into(),
                                    token_info.line_num,
                                    token_info.line_pos,
                                )?;
                                state.parse_include()?;
                                state.includes_num += 1;
                            } else {
                                return Err(token_info.custom(ErrorType::TooManyIncludes));
                            }
                        }
                        Word::Return => {
                            state.validate_argument(
                                0,
                                Capability::Include.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            let mut num_pops = 0;

                            for block in [&state.block]
                                .into_iter()
                                .chain(state.block_stack.iter().rev())
                            {
                                if let Word::ForEveryPart = &block.btype {
                                    num_pops += 1;
                                }
                            }

                            if num_pops > 0 {
                                state
                                    .instructions
                                    .push(Instruction::ForEveryPartPop(num_pops));
                            }

                            state.instructions.push(Instruction::Return);
                        }
                        Word::Global => {
                            state.validate_argument(
                                0,
                                Capability::Include.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.validate_argument(
                                0,
                                Capability::Variables.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            for global in state.parse_static_strings()? {
                                if !state.is_var_local(&global) {
                                    if global.len() < self.max_variable_name_size {
                                        state.register_global_var(&global);
                                    } else {
                                        return Err(state
                                            .tokens
                                            .unwrap_next()?
                                            .custom(ErrorType::VariableTooLong));
                                    }
                                } else {
                                    return Err(state
                                        .tokens
                                        .unwrap_next()?
                                        .custom(ErrorType::VariableIsLocal(global)));
                                }
                            }
                        }

                        // Expressions extension
                        Word::Let => {
                            state.validate_argument(
                                0,
                                Capability::Expressions.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            state.parse_let()?;
                        }
                        Word::Eval => {
                            state.validate_argument(
                                0,
                                Capability::Expressions.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            let expr = state.parse_expr()?;
                            state.instructions.push(Instruction::Eval(expr));
                        }

                        // While extension
                        Word::While => {
                            state.validate_argument(
                                0,
                                Capability::While.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;

                            is_new_block = Block::new(Word::While).into();

                            let expr = state.parse_expr()?;
                            state.instructions.push(Instruction::While(While {
                                expr,
                                jz_pos: usize::MAX,
                            }));
                        }
                        Word::Continue => {
                            state.validate_argument(
                                0,
                                Capability::While.into(),
                                token_info.line_num,
                                token_info.line_pos,
                            )?;
                            let mut found_while = 0;
                            for block in [&state.block]
                                .into_iter()
                                .chain(state.block_stack.iter().rev())
                            {
                                if let Word::While = &block.btype {
                                    found_while += 1;
                                } else if found_while == 1 {
                                    state
                                        .instructions
                                        .push(Instruction::Jmp(block.last_block_start));
                                    found_while += 1;
                                    break;
                                }
                            }
                            if found_while != 2 {
                                return Err(token_info.custom(ErrorType::ContinueOutsideLoop));
                            }
                        }

                        _ => {
                            if state.has_capability(&Capability::Ihave) {
                                state.ignore_instruction()?;
                                state.instructions.push(Instruction::Invalid(Invalid {
                                    name: instruction.to_string(),
                                    line_num: token_info.line_num,
                                    line_pos: token_info.line_pos,
                                }));
                                continue;
                            } else {
                                return Err(CompileError {
                                    line_num: state.block.line_num,
                                    line_pos: state.block.line_pos,
                                    error_type: ErrorType::UnexpectedToken {
                                        expected: "command".into(),
                                        found: instruction.to_string(),
                                    },
                                });
                            }
                        }
                    }

                    if let Some(mut new_block) = is_new_block {
                        new_block.line_num = state.tokens.line_num;
                        new_block.line_pos = state.tokens.pos - state.tokens.line_start;

                        state.tokens.expect_token(Token::CurlyOpen)?;
                        if state.block_stack.len() < self.max_nested_blocks {
                            state.block.last_block_start = state.instructions.len() - 1;
                            state.block_stack.push(state.block);
                            state.block = new_block;
                        } else {
                            return Err(CompileError {
                                line_num: state.block.line_num,
                                line_pos: state.block.line_pos,
                                error_type: ErrorType::TooManyNestedBlocks,
                            });
                        }
                    } else {
                        state.expect_instruction_end()?;
                    }
                }
                Token::CurlyClose if !state.block_stack.is_empty() => {
                    state.block_end();
                    let mut prev_block = state.block_stack.pop().unwrap();
                    match &state.block.btype {
                        Word::ForEveryPart => {
                            state
                                .instructions
                                .push(Instruction::Jmp(prev_block.last_block_start));
                            let cur_pos = state.instructions.len();
                            if let Instruction::ForEveryPart(fep) =
                                &mut state.instructions[prev_block.last_block_start]
                            {
                                fep.jz_pos = cur_pos;
                            } else {
                                debug_assert!(false, "This should not have happened.");
                            }
                            for pos in state.block.break_jmps {
                                if let Instruction::Jmp(jmp_pos) = &mut state.instructions[pos] {
                                    *jmp_pos = cur_pos;
                                } else {
                                    debug_assert!(false, "This should not have happened.");
                                }
                            }
                            state.last_block_type = Word::Not;
                        }
                        Word::If | Word::ElsIf => {
                            let next_is_block = matches!(
                                state.tokens.peek().map(|r| r.map(|t| &t.token)),
                                Some(Ok(Token::Identifier(Word::ElsIf | Word::Else)))
                            );
                            if next_is_block {
                                prev_block.if_jmps.push(state.instructions.len());
                                state.instructions.push(Instruction::Jmp(usize::MAX));
                            }
                            let cur_pos = state.instructions.len();
                            if let Instruction::Jz(jmp_pos) =
                                &mut state.instructions[prev_block.last_block_start]
                            {
                                *jmp_pos = cur_pos;
                            } else {
                                debug_assert!(false, "This should not have happened.");
                            }
                            if !next_is_block {
                                for pos in prev_block.if_jmps.drain(..) {
                                    if let Instruction::Jmp(jmp_pos) = &mut state.instructions[pos]
                                    {
                                        *jmp_pos = cur_pos;
                                    } else {
                                        debug_assert!(false, "This should not have happened.");
                                    }
                                }
                                state.last_block_type = Word::Not;
                            } else {
                                state.last_block_type = state.block.btype;
                            }
                        }
                        Word::Else => {
                            let cur_pos = state.instructions.len();
                            for pos in prev_block.if_jmps.drain(..) {
                                if let Instruction::Jmp(jmp_pos) = &mut state.instructions[pos] {
                                    *jmp_pos = cur_pos;
                                } else {
                                    debug_assert!(false, "This should not have happened.");
                                }
                            }
                            state.last_block_type = Word::Else;
                        }
                        Word::While => {
                            state
                                .instructions
                                .push(Instruction::Jmp(prev_block.last_block_start));
                            let cur_pos = state.instructions.len();
                            if let Instruction::While(fep) =
                                &mut state.instructions[prev_block.last_block_start]
                            {
                                fep.jz_pos = cur_pos;
                            } else {
                                debug_assert!(false, "This should not have happened.");
                            }
                            for pos in state.block.break_jmps {
                                if let Instruction::Jmp(jmp_pos) = &mut state.instructions[pos] {
                                    *jmp_pos = cur_pos;
                                } else {
                                    debug_assert!(false, "This should not have happened.");
                                }
                            }
                            state.last_block_type = Word::Not;
                        }
                        _ => {
                            debug_assert!(false, "This should not have happened.");
                        }
                    }

                    state.block = prev_block;
                }

                #[cfg(test)]
                Token::Unknown(instruction) if instruction.contains("test") => {
                    let has_arguments = instruction != "test";
                    let mut arguments = vec![Value::Text(instruction.into())];

                    if !has_arguments {
                        arguments.push(state.parse_string()?);
                        state.instructions.push(Instruction::TestCmd(arguments));
                        let mut new_block = Block::new(Word::Else);
                        new_block.line_num = state.tokens.line_num;
                        new_block.line_pos = state.tokens.pos - state.tokens.line_start;
                        state.tokens.expect_token(Token::CurlyOpen)?;
                        state.block.last_block_start = state.instructions.len() - 1;
                        state.block_stack.push(state.block);
                        state.block = new_block;
                    } else {
                        loop {
                            arguments.push(match state.tokens.unwrap_next()?.token {
                                Token::StringConstant(s) => Value::from(s),
                                Token::StringVariable(s) => state
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
                                Token::Semicolon => break,
                                other => panic!("Invalid test param {other:?}"),
                            });
                        }
                        state.instructions.push(Instruction::TestCmd(arguments));
                    }
                }

                Token::Unknown(instruction) => {
                    if state.has_capability(&Capability::Ihave) {
                        state.ignore_instruction()?;
                        state.instructions.push(Instruction::Invalid(Invalid {
                            name: instruction,
                            line_num: token_info.line_num,
                            line_pos: token_info.line_pos,
                        }));
                    } else {
                        return Err(CompileError {
                            line_num: state.block.line_num,
                            line_pos: state.block.line_pos,
                            error_type: ErrorType::UnexpectedToken {
                                expected: "command".into(),
                                found: instruction,
                            },
                        });
                    }
                }
                _ => {
                    return Err(token_info.expected("instruction"));
                }
            }
        }

        if !state.block_stack.is_empty() {
            return Err(CompileError {
                line_num: state.block.line_num,
                line_pos: state.block.line_pos,
                error_type: ErrorType::UnterminatedBlock,
            });
        }

        // Map local variables
        let mut num_vars = std::cmp::max(state.vars_num_max, state.vars_num);
        if state.vars_local > 0 {
            state.map_local_vars(num_vars);
            num_vars += state.vars_local;
        }

        Ok(Sieve {
            instructions: state.instructions,
            num_vars: num_vars as u32,
            num_match_vars: state.vars_match_max as u32,
        })
    }
}

impl CompilerState<'_> {
    pub(crate) fn is_var_local(&self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        if self.block.vars_local.contains_key(&name) {
            true
        } else {
            for block in self.block_stack.iter().rev() {
                if block.vars_local.contains_key(&name) {
                    return true;
                }
            }
            false
        }
    }

    pub(crate) fn is_var_global(&self, name: &str) -> bool {
        let name = name.to_ascii_lowercase();
        self.vars_global.contains(&name)
    }

    pub(crate) fn register_local_var(&mut self, name: String, register_as_local: bool) -> usize {
        if let Some(var_id) = self.get_local_var(&name) {
            var_id
        } else if !register_as_local || self.block_stack.is_empty() {
            let var_id = self.vars_num;
            self.block.vars_local.insert(name, var_id);
            self.vars_num += 1;
            var_id
        } else {
            let var_id = usize::MAX - self.vars_local;
            self.block_stack
                .first_mut()
                .unwrap()
                .vars_local
                .insert(name, usize::MAX - self.vars_local);
            self.vars_local += 1;
            var_id
        }
    }

    pub(crate) fn register_global_var(&mut self, name: &str) {
        self.vars_global.insert(name.to_ascii_lowercase());
    }

    pub(crate) fn get_local_var(&self, name: &str) -> Option<usize> {
        let name = name.to_ascii_lowercase();
        if let Some(var_id) = self.block.vars_local.get(&name) {
            Some(*var_id)
        } else {
            for block in self.block_stack.iter().rev() {
                if let Some(var_id) = block.vars_local.get(&name) {
                    return Some(*var_id);
                }
            }
            None
        }
    }

    pub(crate) fn register_match_var(&mut self, num: usize) -> bool {
        let mut block = &mut self.block;

        if block.match_test_pos.is_empty() {
            for block_ in self.block_stack.iter_mut().rev() {
                if !block_.match_test_pos.is_empty() {
                    block = block_;
                    break;
                }
            }
        }

        if !block.match_test_pos.is_empty() {
            debug_assert!(num < 63);

            for pos in &block.match_test_pos {
                if let Some(Instruction::Test(test)) = self.instructions.get_mut(*pos) {
                    let match_type = match test {
                        Test::Address(t) => &mut t.match_type,
                        Test::Body(t) => &mut t.match_type,
                        Test::Date(t) => &mut t.match_type,
                        Test::CurrentDate(t) => &mut t.match_type,
                        Test::Envelope(t) => &mut t.match_type,
                        Test::HasFlag(t) => &mut t.match_type,
                        Test::Header(t) => &mut t.match_type,
                        Test::Metadata(t) => &mut t.match_type,
                        Test::NotifyMethodCapability(t) => &mut t.match_type,
                        Test::SpamTest(t) => &mut t.match_type,
                        Test::String(t) | Test::Environment(t) => &mut t.match_type,
                        Test::VirusTest(t) => &mut t.match_type,
                        _ => {
                            debug_assert!(false, "This should not have happened: {test:?}");
                            return false;
                        }
                    };
                    if let MatchType::Matches(positions) | MatchType::Regex(positions) = match_type
                    {
                        *positions |= 1 << num;
                        block.match_test_vars = *positions;
                    } else {
                        debug_assert!(false, "This should not have happened");
                        return false;
                    }
                } else {
                    debug_assert!(false, "This should not have happened");
                    return false;
                }
            }
            true
        } else {
            false
        }
    }

    pub(crate) fn block_end(&mut self) {
        let vars_num_block = self.block.vars_local.len();
        if vars_num_block > 0 {
            if self.vars_num > self.vars_num_max {
                self.vars_num_max = self.vars_num;
            }
            self.vars_num -= vars_num_block;
            self.instructions.push(Instruction::Clear(Clear {
                match_vars: self.block.match_test_vars,
                local_vars_idx: self.vars_num as u32,
                local_vars_num: vars_num_block as u32,
            }));
        } else if self.block.match_test_vars != 0 {
            self.instructions.push(Instruction::Clear(Clear {
                match_vars: self.block.match_test_vars,
                local_vars_idx: 0,
                local_vars_num: 0,
            }));
        }
    }

    fn map_local_vars(&mut self, last_id: usize) {
        for instruction in &mut self.instructions {
            match instruction {
                Instruction::Test(v) => v.map_local_vars(last_id),
                Instruction::Keep(k) => k.flags.map_local_vars(last_id),
                Instruction::FileInto(v) => {
                    v.folder.map_local_vars(last_id);
                    v.flags.map_local_vars(last_id);
                    v.mailbox_id.map_local_vars(last_id);
                    v.special_use.map_local_vars(last_id);
                }
                Instruction::Redirect(v) => {
                    v.address.map_local_vars(last_id);
                    v.by_time.map_local_vars(last_id);
                }
                Instruction::Replace(v) => {
                    v.subject.map_local_vars(last_id);
                    v.from.map_local_vars(last_id);
                    v.replacement.map_local_vars(last_id);
                }
                Instruction::Enclose(v) => {
                    v.subject.map_local_vars(last_id);
                    v.headers.map_local_vars(last_id);
                    v.value.map_local_vars(last_id);
                }
                Instruction::ExtractText(v) => {
                    v.name.map_local_vars(last_id);
                }
                Instruction::Convert(v) => {
                    v.from_media_type.map_local_vars(last_id);
                    v.to_media_type.map_local_vars(last_id);
                    v.transcoding_params.map_local_vars(last_id);
                }
                Instruction::AddHeader(v) => {
                    v.field_name.map_local_vars(last_id);
                    v.value.map_local_vars(last_id);
                }
                Instruction::DeleteHeader(v) => {
                    v.field_name.map_local_vars(last_id);
                    v.value_patterns.map_local_vars(last_id);
                }
                Instruction::Set(v) => {
                    v.name.map_local_vars(last_id);
                    v.value.map_local_vars(last_id);
                }
                Instruction::Let(v) => {
                    v.name.map_local_vars(last_id);
                    v.expr.map_local_vars(last_id);
                }
                Instruction::While(v) => {
                    v.expr.map_local_vars(last_id);
                }
                Instruction::Eval(v) => {
                    v.map_local_vars(last_id);
                }
                Instruction::Notify(v) => {
                    v.from.map_local_vars(last_id);
                    v.importance.map_local_vars(last_id);
                    v.options.map_local_vars(last_id);
                    v.message.map_local_vars(last_id);
                    v.fcc.map_local_vars(last_id);
                    v.method.map_local_vars(last_id);
                }
                Instruction::Reject(v) => {
                    v.reason.map_local_vars(last_id);
                }
                Instruction::Vacation(v) => {
                    v.subject.map_local_vars(last_id);
                    v.from.map_local_vars(last_id);
                    v.fcc.map_local_vars(last_id);
                    v.reason.map_local_vars(last_id);
                }
                Instruction::Error(v) => {
                    v.message.map_local_vars(last_id);
                }
                Instruction::EditFlags(v) => {
                    v.name.map_local_vars(last_id);
                    v.flags.map_local_vars(last_id);
                }
                Instruction::Include(v) => {
                    v.value.map_local_vars(last_id);
                }
                _ => {}
            }
        }
    }
}

pub trait MapLocalVars {
    fn map_local_vars(&mut self, last_id: usize);
}

impl MapLocalVars for Test {
    fn map_local_vars(&mut self, last_id: usize) {
        match self {
            Test::Address(v) => {
                v.header_list.map_local_vars(last_id);
                v.key_list.map_local_vars(last_id);
            }
            Test::Envelope(v) => {
                v.key_list.map_local_vars(last_id);
            }
            Test::Exists(v) => {
                v.header_names.map_local_vars(last_id);
            }
            Test::Header(v) => {
                v.key_list.map_local_vars(last_id);
                v.header_list.map_local_vars(last_id);
                v.mime_opts.map_local_vars(last_id);
            }
            Test::Body(v) => {
                v.key_list.map_local_vars(last_id);
            }
            Test::Convert(v) => {
                v.from_media_type.map_local_vars(last_id);
                v.to_media_type.map_local_vars(last_id);
                v.transcoding_params.map_local_vars(last_id);
            }
            Test::Date(v) => {
                v.key_list.map_local_vars(last_id);
                v.header_name.map_local_vars(last_id);
            }
            Test::CurrentDate(v) => {
                v.key_list.map_local_vars(last_id);
            }
            Test::Duplicate(v) => {
                v.handle.map_local_vars(last_id);
                v.dup_match.map_local_vars(last_id);
            }
            Test::String(v) => {
                v.source.map_local_vars(last_id);
                v.key_list.map_local_vars(last_id);
            }
            Test::Environment(v) => {
                v.source.map_local_vars(last_id);
                v.key_list.map_local_vars(last_id);
            }
            Test::NotifyMethodCapability(v) => {
                v.key_list.map_local_vars(last_id);
                v.notification_capability.map_local_vars(last_id);
                v.notification_uri.map_local_vars(last_id);
            }
            Test::ValidNotifyMethod(v) => {
                v.notification_uris.map_local_vars(last_id);
            }
            Test::ValidExtList(v) => {
                v.list_names.map_local_vars(last_id);
            }
            Test::HasFlag(v) => {
                v.variable_list.map_local_vars(last_id);
                v.flags.map_local_vars(last_id);
            }
            Test::MailboxExists(v) => {
                v.mailbox_names.map_local_vars(last_id);
            }
            Test::Metadata(v) => {
                v.key_list.map_local_vars(last_id);
                v.medatata.map_local_vars(last_id);
            }
            Test::MetadataExists(v) => {
                v.annotation_names.map_local_vars(last_id);
                v.mailbox.map_local_vars(last_id);
            }
            Test::MailboxIdExists(v) => {
                v.mailbox_ids.map_local_vars(last_id);
            }
            Test::SpamTest(v) => {
                v.value.map_local_vars(last_id);
            }
            Test::VirusTest(v) => {
                v.value.map_local_vars(last_id);
            }
            Test::SpecialUseExists(v) => {
                v.mailbox.map_local_vars(last_id);
                v.attributes.map_local_vars(last_id);
            }
            Test::Vacation(v) => {
                v.addresses.map_local_vars(last_id);
                v.handle.map_local_vars(last_id);
                v.reason.map_local_vars(last_id);
            }
            #[cfg(test)]
            Test::TestCmd { arguments, .. } => {
                arguments.map_local_vars(last_id);
            }
            _ => (),
        }
    }
}

impl MapLocalVars for VariableType {
    fn map_local_vars(&mut self, last_id: usize) {
        match self {
            VariableType::Local(id) if *id > last_id => {
                *id = (usize::MAX - *id) + last_id;
            }
            _ => (),
        }
    }
}

impl MapLocalVars for Value {
    fn map_local_vars(&mut self, last_id: usize) {
        match self {
            Value::Variable(var) => var.map_local_vars(last_id),
            Value::List(items) => items.map_local_vars(last_id),
            _ => (),
        }
    }
}

impl<T: MapLocalVars> MapLocalVars for Option<T> {
    fn map_local_vars(&mut self, last_id: usize) {
        if let Some(value) = self {
            value.map_local_vars(last_id);
        }
    }
}

impl MapLocalVars for Expression {
    fn map_local_vars(&mut self, last_id: usize) {
        if let Expression::Variable(var) = self {
            var.map_local_vars(last_id)
        }
    }
}

impl<T: MapLocalVars> MapLocalVars for Vec<T> {
    fn map_local_vars(&mut self, last_id: usize) {
        for item in self {
            item.map_local_vars(last_id);
        }
    }
}

impl Block {
    pub fn new(btype: Word) -> Self {
        Block {
            btype,
            label: None,
            line_num: 0,
            line_pos: 0,
            last_block_start: 0,
            match_test_pos: vec![],
            match_test_vars: 0,
            if_jmps: vec![],
            break_jmps: vec![],
            vars_local: AHashMap::new(),
            capabilities: AHashSet::new(),
            require_pos: usize::MAX,
        }
    }

    pub fn with_label(mut self, label: String) -> Self {
        self.label = label.into();
        self
    }
}
