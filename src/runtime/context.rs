/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::{borrow::Cow, sync::Arc, time::SystemTime};

use ahash::AHashMap;
use mail_parser::Message;

use crate::{
    compiler::grammar::{instruction::Instruction, Capability},
    Context, Envelope, Event, Input, Metadata, Runtime, Sieve, SpamStatus, VirusStatus,
    MAX_LOCAL_VARIABLES, MAX_MATCH_VARIABLES,
};

use super::{
    actions::action_include::IncludeResult,
    tests::{test_envelope::parse_envelope_address, TestResult},
    RuntimeError, Variable,
};

#[derive(Clone, Debug)]
pub(crate) struct ScriptStack {
    pub(crate) script: Arc<Sieve>,
    pub(crate) prev_pos: usize,
    pub(crate) prev_vars_local: Vec<Variable>,
    pub(crate) prev_vars_match: Vec<Variable>,
}

impl<'x> Context<'x> {
    #[cfg(not(test))]
    pub(crate) fn new(runtime: &'x Runtime, message: Message<'x>) -> Self {
        Context {
            #[cfg(test)]
            runtime: runtime.clone(),
            #[cfg(not(test))]
            runtime,
            message,
            part: 0,
            part_iter: Vec::new().into_iter(),
            part_iter_stack: Vec::new(),
            pos: usize::MAX,
            test_result: false,
            script_cache: AHashMap::new(),
            script_stack: Vec::with_capacity(0),
            vars_global: AHashMap::new(),
            vars_env: AHashMap::new(),
            vars_local: Vec::with_capacity(0),
            vars_match: Vec::with_capacity(0),
            expr_stack: Vec::with_capacity(16),
            expr_pos: 0,
            envelope: Vec::new(),
            metadata: Vec::new(),
            message_size: usize::MAX,
            final_event: Event::Keep {
                flags: Vec::with_capacity(0),
                message_id: 0,
            }
            .into(),
            queued_events: vec![].into_iter(),
            has_changes: false,
            user_address: "".into(),
            user_full_name: "".into(),
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0) as i64,
            num_redirects: 0,
            num_instructions: 0,
            num_out_messages: 0,
            last_message_id: 0,
            main_message_id: 0,
            virus_status: VirusStatus::Unknown,
            spam_status: SpamStatus::Unknown,
        }
    }

    #[allow(clippy::while_let_on_iterator)]
    pub fn run(&mut self, input: Input) -> Option<Result<Event, RuntimeError>> {
        match input {
            Input::True => self.test_result ^= true,
            Input::False => self.test_result ^= false,
            Input::FncResult(result) => {
                self.expr_stack.push(result);
            }
            Input::Script { name, script } => {
                let num_vars = script.num_vars;
                let num_match_vars = script.num_match_vars;

                if num_match_vars <= MAX_MATCH_VARIABLES && num_vars <= MAX_LOCAL_VARIABLES {
                    if self.message_size == usize::MAX {
                        self.message_size = self.message.raw_message.len();
                    }

                    self.script_cache.insert(name, script.clone());
                    self.script_stack.push(ScriptStack {
                        script,
                        prev_pos: self.pos,
                        prev_vars_local: std::mem::replace(
                            &mut self.vars_local,
                            vec![Variable::default(); num_vars as usize],
                        ),
                        prev_vars_match: std::mem::replace(
                            &mut self.vars_match,
                            vec![Variable::default(); num_match_vars as usize],
                        ),
                    });
                    self.pos = 0;
                    self.test_result = false;
                }
            }
        }

        // Return any queued events
        if let Some(event) = self.queued_events.next() {
            return Some(Ok(event));
        }

        let mut current_script = self.script_stack.last()?.script.clone();
        let mut iter = current_script.instructions.get(self.pos..)?.iter();

        'outer: loop {
            while let Some(instruction) = iter.next() {
                self.num_instructions += 1;
                if self.num_instructions > self.runtime.cpu_limit {
                    self.finish_loop();
                    return Some(Err(RuntimeError::CPULimitReached));
                }
                self.pos += 1;

                match instruction {
                    Instruction::Jz(jmp_pos) => {
                        if !self.test_result {
                            debug_assert!(*jmp_pos > self.pos - 1);
                            self.pos = *jmp_pos;
                            iter = current_script.instructions.get(self.pos..)?.iter();
                            continue;
                        }
                    }
                    Instruction::Jnz(jmp_pos) => {
                        if self.test_result {
                            debug_assert!(*jmp_pos > self.pos - 1);
                            self.pos = *jmp_pos;
                            iter = current_script.instructions.get(self.pos..)?.iter();
                            continue;
                        }
                    }
                    Instruction::Jmp(jmp_pos) => {
                        debug_assert_ne!(*jmp_pos, self.pos - 1);
                        self.pos = *jmp_pos;
                        iter = current_script.instructions.get(self.pos..)?.iter();
                        continue;
                    }
                    Instruction::Test(test) => match test.exec(self) {
                        TestResult::Bool(result) => {
                            self.test_result = result;
                        }
                        TestResult::Event { event, is_not } => {
                            self.test_result = is_not;
                            return Some(Ok(event));
                        }
                        TestResult::Error(err) => {
                            self.finish_loop();
                            return Some(Err(err));
                        }
                    },
                    Instruction::Eval(expr) => match self.eval_expression(expr) {
                        Ok(result) => {
                            self.test_result = result.to_bool();
                        }
                        Err(event) => {
                            return Some(Ok(event));
                        }
                    },
                    Instruction::Clear(clear) => {
                        if clear.local_vars_num > 0 {
                            if let Some(local_vars) = self.vars_local.get_mut(
                                clear.local_vars_idx as usize
                                    ..(clear.local_vars_idx + clear.local_vars_num) as usize,
                            ) {
                                for local_var in local_vars.iter_mut() {
                                    if !local_var.is_empty() {
                                        *local_var = Variable::default();
                                    }
                                }
                            } else {
                                debug_assert!(false, "Failed to clear local variables: {clear:?}");
                            }
                        }
                        if clear.match_vars != 0 {
                            self.clear_match_variables(clear.match_vars);
                        }
                    }
                    Instruction::Keep(keep) => {
                        let next_event = self.build_message_id();
                        self.final_event = Event::Keep {
                            flags: self.get_local_or_global_flags(&keep.flags),
                            message_id: self.main_message_id,
                        }
                        .into();
                        if let Some(next_event) = next_event {
                            return Some(Ok(next_event));
                        }
                    }
                    Instruction::FileInto(fi) => {
                        fi.exec(self);
                        if let Some(event) = self.queued_events.next() {
                            return Some(Ok(event));
                        }
                    }
                    Instruction::Redirect(redirect) => {
                        redirect.exec(self);
                        if let Some(event) = self.queued_events.next() {
                            return Some(Ok(event));
                        }
                    }
                    Instruction::Discard => {
                        self.final_event = Event::Discard.into();
                    }
                    Instruction::Stop => {
                        self.script_stack.clear();
                        break 'outer;
                    }
                    Instruction::Reject(reject) => {
                        self.final_event = None;
                        return Some(Ok(Event::Reject {
                            extended: reject.ereject,
                            reason: self.eval_value(&reject.reason).to_string().into_owned(),
                        }));
                    }
                    Instruction::ForEveryPart(fep) => {
                        if let Some(next_part) = self.part_iter.next() {
                            self.part = next_part;
                        } else if let Some((prev_part, prev_part_iter)) = self.part_iter_stack.pop()
                        {
                            debug_assert!(fep.jz_pos > self.pos - 1);
                            self.part_iter = prev_part_iter;
                            self.part = prev_part;
                            self.pos = fep.jz_pos;
                            iter = current_script.instructions.get(self.pos..)?.iter();
                            continue;
                        } else {
                            self.part = 0;
                            #[cfg(test)]
                            panic!("ForEveryPart executed without items on stack.");
                        }
                    }
                    Instruction::ForEveryPartPush => {
                        let part_iter = self
                            .find_nested_parts_ids(self.part_iter_stack.is_empty())
                            .into_iter();
                        self.part_iter_stack
                            .push((self.part, std::mem::replace(&mut self.part_iter, part_iter)));
                    }
                    Instruction::ForEveryPartPop(num_pops) => {
                        debug_assert!(
                            *num_pops > 0 && *num_pops <= self.part_iter_stack.len(),
                            "Pop out of range: {} with {} items.",
                            num_pops,
                            self.part_iter_stack.len()
                        );
                        for _ in 0..*num_pops {
                            if let Some((prev_part, prev_part_iter)) = self.part_iter_stack.pop() {
                                self.part_iter = prev_part_iter;
                                self.part = prev_part;
                            } else {
                                break;
                            }
                        }
                    }
                    Instruction::While(while_) => match self.eval_expression(&while_.expr) {
                        Ok(result) => {
                            if !result.to_bool() {
                                debug_assert!(while_.jz_pos > self.pos - 1);
                                self.pos = while_.jz_pos;
                                iter = current_script.instructions.get(self.pos..)?.iter();
                                continue;
                            }
                        }
                        Err(event) => {
                            return Some(Ok(event));
                        }
                    },
                    Instruction::Let(let_) => match self.eval_expression(&let_.expr) {
                        Ok(result) => {
                            self.set_variable(&let_.name, result);
                        }
                        Err(event) => {
                            return Some(Ok(event));
                        }
                    },

                    Instruction::Replace(replace) => replace.exec(self),
                    Instruction::Enclose(enclose) => enclose.exec(self),
                    Instruction::ExtractText(extract) => {
                        extract.exec(self);
                        if let Some(event) = self.queued_events.next() {
                            return Some(Ok(event));
                        }
                    }
                    Instruction::AddHeader(add_header) => add_header.exec(self),
                    Instruction::DeleteHeader(delete_header) => delete_header.exec(self),
                    Instruction::Set(set) => {
                        set.exec(self);
                        if let Some(event) = self.queued_events.next() {
                            return Some(Ok(event));
                        }
                    }
                    Instruction::Notify(notify) => {
                        notify.exec(self);
                        if let Some(event) = self.queued_events.next() {
                            return Some(Ok(event));
                        }
                    }
                    Instruction::Vacation(vacation) => {
                        vacation.exec(self);
                        if let Some(event) = self.queued_events.next() {
                            return Some(Ok(event));
                        }
                    }
                    Instruction::EditFlags(flags) => flags.exec(self),
                    Instruction::Include(include) => match include.exec(self) {
                        IncludeResult::Cached(script) => {
                            self.script_stack.push(ScriptStack {
                                script: script.clone(),
                                prev_pos: self.pos,
                                prev_vars_local: std::mem::replace(
                                    &mut self.vars_local,
                                    vec![Variable::default(); script.num_vars as usize],
                                ),
                                prev_vars_match: std::mem::replace(
                                    &mut self.vars_match,
                                    vec![Variable::default(); script.num_match_vars as usize],
                                ),
                            });
                            self.pos = 0;
                            current_script = script;
                            iter = current_script.instructions.iter();
                            continue;
                        }
                        IncludeResult::Event(event) => {
                            return Some(Ok(event));
                        }
                        IncludeResult::Error(err) => {
                            self.finish_loop();
                            return Some(Err(err));
                        }
                        IncludeResult::None => (),
                    },
                    Instruction::Convert(convert) => {
                        convert.exec(self);
                    }
                    Instruction::Return => {
                        break;
                    }
                    Instruction::Require(capabilities) => {
                        for capability in capabilities {
                            if !self.runtime.allowed_capabilities.contains(capability) {
                                self.finish_loop();
                                return Some(Err(
                                    if let Capability::Other(not_supported) = capability {
                                        RuntimeError::CapabilityNotSupported(not_supported.clone())
                                    } else {
                                        RuntimeError::CapabilityNotAllowed(capability.clone())
                                    },
                                ));
                            }
                        }
                    }
                    Instruction::Error(err) => {
                        self.finish_loop();
                        return Some(Err(RuntimeError::ScriptErrorMessage(
                            self.eval_value(&err.message).to_string().into_owned(),
                        )));
                    }
                    Instruction::Invalid(invalid) => {
                        self.finish_loop();
                        return Some(Err(RuntimeError::InvalidInstruction(invalid.clone())));
                    }
                    #[cfg(test)]
                    Instruction::TestCmd(arguments) => {
                        return Some(Ok(Event::Function {
                            id: u32::MAX,
                            arguments: arguments
                                .iter()
                                .map(|s| self.eval_value(s).to_owned())
                                .collect(),
                        }));
                    }
                }
            }

            if let Some(prev_script) = self.script_stack.pop() {
                self.pos = prev_script.prev_pos;
                self.vars_local = prev_script.prev_vars_local;
                self.vars_match = prev_script.prev_vars_match;
            }

            if let Some(script_stack) = self.script_stack.last() {
                current_script = script_stack.script.clone();
                iter = current_script.instructions.get(self.pos..)?.iter();
            } else {
                break;
            }
        }

        match self.final_event.take() {
            Some(Event::Keep {
                mut flags,
                message_id,
            }) => {
                let create_event = if self.has_changes {
                    self.build_message_id()
                } else {
                    None
                };

                let global_flags = self.get_global_flags();
                if flags.is_empty() && !global_flags.is_empty() {
                    flags = global_flags;
                }
                if let Some(create_event) = create_event {
                    self.queued_events = vec![
                        create_event,
                        Event::Keep {
                            flags,
                            message_id: self.main_message_id,
                        },
                    ]
                    .into_iter();
                    self.queued_events.next().map(Ok)
                } else {
                    Some(Ok(Event::Keep { flags, message_id }))
                }
            }
            Some(event) => Some(Ok(event)),
            _ => None,
        }
    }

    pub(crate) fn finish_loop(&mut self) {
        self.script_stack.clear();
        if let Some(event) = self.final_event.take() {
            self.queued_events = if let Event::Keep {
                mut flags,
                message_id,
            } = event
            {
                let global_flags = self.get_global_flags();
                if flags.is_empty() && !global_flags.is_empty() {
                    flags = global_flags;
                }

                if self.has_changes {
                    if let Some(event) = self.build_message_id() {
                        vec![
                            event,
                            Event::Keep {
                                flags,
                                message_id: self.main_message_id,
                            },
                        ]
                    } else {
                        vec![Event::Keep { flags, message_id }]
                    }
                } else {
                    vec![Event::Keep { flags, message_id }]
                }
            } else {
                vec![event]
            }
            .into_iter();
        }
    }

    pub fn set_envelope(
        &mut self,
        envelope: impl TryInto<Envelope>,
        value: impl Into<Cow<'x, str>>,
    ) {
        if let Ok(envelope) = envelope.try_into() {
            if matches!(&envelope, Envelope::From | Envelope::To) {
                let value: Cow<str> = value.into();
                if let Some(value) = parse_envelope_address(value.as_ref()) {
                    self.envelope.push((envelope, value.to_string().into()));
                }
            } else {
                self.envelope.push((envelope, Variable::from(value.into())));
            }
        }
    }

    pub fn with_vars_env(mut self, vars_env: AHashMap<Cow<'static, str>, Variable>) -> Self {
        self.vars_env = vars_env;
        self
    }

    pub fn with_envelope_list(mut self, envelope: Vec<(Envelope, Variable)>) -> Self {
        self.envelope = envelope;
        self
    }

    pub fn with_envelope(
        mut self,
        envelope: impl TryInto<Envelope>,
        value: impl Into<Cow<'x, str>>,
    ) -> Self {
        self.set_envelope(envelope, value);
        self
    }

    pub fn clear_envelope(&mut self) {
        self.envelope.clear()
    }

    pub fn set_user_address(&mut self, from: impl Into<Cow<'x, str>>) {
        self.user_address = from.into();
    }

    pub fn with_user_address(mut self, from: impl Into<Cow<'x, str>>) -> Self {
        self.set_user_address(from);
        self
    }

    pub fn set_user_full_name(&mut self, name: &str) {
        let mut name_ = String::with_capacity(name.len());
        for ch in name.chars() {
            if ['\"', '\\'].contains(&ch) {
                name_.push('\\');
            }
            name_.push(ch);
        }
        self.user_full_name = name_.into();
    }

    pub fn with_user_full_name(mut self, name: &str) -> Self {
        self.set_user_full_name(name);
        self
    }

    pub fn set_env_variable(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Variable>,
    ) {
        self.vars_env.insert(name.into(), value.into());
    }

    pub fn with_env_variable(
        mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Variable>,
    ) -> Self {
        self.set_env_variable(name, value);
        self
    }

    pub fn set_global_variable(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Variable>,
    ) {
        self.vars_global.insert(name.into(), value.into());
    }

    pub fn with_global_variable(
        mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Variable>,
    ) -> Self {
        self.set_global_variable(name, value);
        self
    }

    pub fn set_medatata(
        &mut self,
        name: impl Into<Metadata<String>>,
        value: impl Into<Cow<'x, str>>,
    ) {
        self.metadata.push((name.into(), value.into()));
    }

    pub fn with_metadata(
        mut self,
        name: impl Into<Metadata<String>>,
        value: impl Into<Cow<'x, str>>,
    ) -> Self {
        self.set_medatata(name, value);
        self
    }

    pub fn set_spam_status(&mut self, status: impl Into<SpamStatus>) {
        self.spam_status = status.into();
    }

    pub fn with_spam_status(mut self, status: impl Into<SpamStatus>) -> Self {
        self.set_spam_status(status);
        self
    }

    pub fn set_virus_status(&mut self, status: impl Into<VirusStatus>) {
        self.virus_status = status.into();
    }

    pub fn with_virus_status(mut self, status: impl Into<VirusStatus>) -> Self {
        self.set_virus_status(status);
        self
    }

    pub fn take_message(&mut self) -> Message<'x> {
        std::mem::take(&mut self.message)
    }

    pub fn has_message_changed(&self) -> bool {
        self.main_message_id > 0
    }

    pub(crate) fn user_from_field(&self) -> String {
        if !self.user_full_name.is_empty() {
            format!("\"{}\" <{}>", self.user_full_name, self.user_address)
        } else {
            self.user_address.to_string()
        }
    }

    pub fn global_variable_names(&self) -> impl Iterator<Item = &str> {
        self.vars_global.keys().map(|k| k.as_ref())
    }

    pub fn global_variable(&self, name: &str) -> Option<&Variable> {
        self.vars_global.get(name)
    }

    pub fn message(&self) -> &Message<'x> {
        &self.message
    }

    pub fn part(&self) -> u32 {
        self.part
    }
}

#[cfg(test)]
impl<'x> Context<'x> {
    pub(crate) fn new(runtime: &'x Runtime, message: Message<'x>) -> Self {
        Context {
            runtime: runtime.clone(),
            message,
            part: 0,
            part_iter: Vec::new().into_iter(),
            part_iter_stack: Vec::new(),
            pos: usize::MAX,
            test_result: false,
            script_cache: AHashMap::new(),
            script_stack: Vec::with_capacity(0),
            vars_global: AHashMap::new(),
            vars_env: AHashMap::new(),
            vars_local: Vec::with_capacity(0),
            vars_match: Vec::with_capacity(0),
            expr_stack: Vec::with_capacity(16),
            expr_pos: 0,
            envelope: Vec::new(),
            metadata: Vec::new(),
            message_size: usize::MAX,
            final_event: Event::Keep {
                flags: Vec::with_capacity(0),
                message_id: 0,
            }
            .into(),
            queued_events: vec![].into_iter(),
            has_changes: false,
            user_address: "".into(),
            user_full_name: "".into(),
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0) as i64,
            num_redirects: 0,
            num_instructions: 0,
            num_out_messages: 0,
            last_message_id: 0,
            main_message_id: 0,
            virus_status: VirusStatus::Unknown,
            spam_status: SpamStatus::Unknown,
        }
    }
}
