use std::{borrow::Cow, sync::Arc, time::SystemTime};

use ahash::AHashMap;
use mail_parser::{Encoding, Message, MessagePart, PartType};

use crate::{
    compiler::grammar::{instruction::Instruction, Capability},
    Action, Context, Envelope, Event, Input, Metadata, Runtime, Sieve, SpamStatus, VirusStatus,
    MAX_LOCAL_VARIABLES, MAX_MATCH_VARIABLES,
};

use super::{
    actions::action_include::IncludeResult,
    tests::{test_envelope::parse_envelope_address, TestResult},
    RuntimeError,
};

#[derive(Clone, Debug)]
pub(crate) struct ScriptStack {
    pub(crate) script: Arc<Sieve>,
    pub(crate) prev_pos: usize,
    pub(crate) prev_vars_local: Vec<String>,
    pub(crate) prev_vars_match: Vec<String>,
}

impl<'x> Context<'x> {
    pub(crate) fn new(runtime: &'x Runtime, raw_message: &'x [u8]) -> Self {
        Context {
            #[cfg(test)]
            runtime: runtime.clone(),
            #[cfg(not(test))]
            runtime,
            message: Message::parse(raw_message).unwrap_or_else(|| Message {
                parts: vec![MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: PartType::Text("".into()),
                    encoding: Encoding::None,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                }],
                raw_message: b""[..].into(),
                ..Default::default()
            }),
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
            envelope: Vec::new(),
            metadata: Vec::new(),
            message_size: usize::MAX,
            actions: vec![Action::Keep {
                flags: Vec::with_capacity(0),
                message_id: 0,
            }],
            has_changes: false,
            user_address: "".into(),
            user_full_name: "".into(),
            current_time: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0) as i64,
            num_redirects: 0,
            num_instructions: 0,
            messages: vec![raw_message.into()],
            last_message_id: 0,
            virus_status: VirusStatus::Unknown,
            spam_status: SpamStatus::Unknown,
        }
    }

    #[allow(clippy::while_let_on_iterator)]
    pub fn run(&mut self, input: Input) -> Option<Result<Event, RuntimeError>> {
        match input {
            Input::True => self.test_result ^= true,
            Input::False => self.test_result ^= false,
            Input::Script { name, script } => {
                let num_vars = script.num_vars;
                let num_match_vars = script.num_match_vars;

                if num_match_vars > MAX_MATCH_VARIABLES || num_vars > MAX_LOCAL_VARIABLES {
                    return Some(Err(RuntimeError::IllegalAction));
                }

                if self.message_size == usize::MAX {
                    self.message_size = self.message.raw_message.len();
                }

                self.script_cache.insert(name, script.clone());
                self.script_stack.push(ScriptStack {
                    script,
                    prev_pos: self.pos,
                    prev_vars_local: std::mem::replace(
                        &mut self.vars_local,
                        vec![String::with_capacity(0); num_vars],
                    ),
                    prev_vars_match: std::mem::replace(
                        &mut self.vars_match,
                        vec![String::with_capacity(0); num_match_vars],
                    ),
                });
                self.pos = 0;
                self.test_result = false;
            }
        }

        let mut current_script = self.script_stack.last()?.script.clone();
        let mut iter = current_script.instructions.get(self.pos..)?.iter();

        'outer: loop {
            while let Some(instruction) = iter.next() {
                self.num_instructions += 1;
                if self.num_instructions > self.runtime.cpu_limit {
                    return Some(Err(RuntimeError::CPULimitReached));
                }

                match instruction {
                    Instruction::Jz(jmp_pos) => {
                        if !self.test_result {
                            debug_assert!(*jmp_pos > self.pos);
                            self.pos = *jmp_pos;
                            iter = current_script.instructions.get(self.pos..)?.iter();
                            continue;
                        }
                    }
                    Instruction::Jnz(jmp_pos) => {
                        if self.test_result {
                            debug_assert!(*jmp_pos > self.pos);
                            self.pos = *jmp_pos;
                            iter = current_script.instructions.get(self.pos..)?.iter();
                            continue;
                        }
                    }
                    Instruction::Jmp(jmp_pos) => {
                        debug_assert_ne!(*jmp_pos, self.pos);
                        self.pos = *jmp_pos;
                        iter = current_script.instructions.get(self.pos..)?.iter();
                        continue;
                    }
                    Instruction::Test(test) => match test.exec(self) {
                        TestResult::Bool(result) => {
                            self.test_result = result;
                        }
                        TestResult::Event { event, is_not } => {
                            self.pos += 1;
                            self.test_result = is_not;
                            return Some(Ok(event));
                        }
                        TestResult::Error(err) => {
                            return Some(Err(err));
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
                                        *local_var = String::with_capacity(0);
                                    }
                                }
                            } else {
                                debug_assert!(
                                    false,
                                    "Failed to clear local variables: {:?}",
                                    clear
                                );
                            }
                        }
                        if clear.match_vars != 0 {
                            self.clear_match_variables(clear.match_vars);
                        }
                    }
                    Instruction::Keep(keep) => {
                        let message_id = match self.build_message_id() {
                            Ok(message_id_) => message_id_,
                            Err(err) => return Some(Err(err)),
                        };
                        self.actions
                            .retain(|a| !matches!(a, Action::Keep { .. } | Action::Discard));
                        self.actions.push(Action::Keep {
                            flags: self.get_local_or_global_flags(&keep.flags),
                            message_id,
                        });
                    }
                    Instruction::FileInto(fi) => {
                        if let Err(err) = fi.exec(self) {
                            return Some(Err(err));
                        }
                    }
                    Instruction::Redirect(redirect) => {
                        if let Err(err) = redirect.exec(self) {
                            return Some(Err(err));
                        }
                    }
                    Instruction::Discard => {
                        self.actions
                            .retain(|a| !matches!(a, Action::Keep { .. } | Action::Discard));
                        self.actions.push(Action::Discard);
                    }
                    Instruction::Stop => {
                        self.script_stack.clear();
                        break 'outer;
                    }
                    Instruction::Reject(reject) => {
                        let reason = self.eval_string(&reject.reason).into_owned();
                        self.actions
                            .retain(|a| !matches!(a, Action::Keep { .. } | Action::Discard));
                        self.actions.push(if reject.ereject {
                            Action::Ereject { reason }
                        } else {
                            Action::Reject { reason }
                        });
                    }
                    Instruction::ForEveryPart(fep) => {
                        if let Some(next_part) = self.part_iter.next() {
                            self.part = next_part;
                        } else if let Some((prev_part, prev_part_iter)) = self.part_iter_stack.pop()
                        {
                            debug_assert!(fep.jz_pos > self.pos);
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
                    Instruction::Replace(replace) => replace.exec(self),
                    Instruction::Enclose(enclose) => enclose.exec(self),
                    Instruction::ExtractText(extract) => extract.exec(self),
                    Instruction::AddHeader(add_header) => add_header.exec(self),
                    Instruction::DeleteHeader(delete_header) => delete_header.exec(self),
                    Instruction::Set(set) => set.exec(self),
                    Instruction::Notify(notify) => {
                        if let Err(err) = notify.exec(self) {
                            return Some(Err(err));
                        }
                    }
                    Instruction::Vacation(vacation) => {
                        if let Err(err) = vacation.exec(self) {
                            return Some(Err(err));
                        }
                    }
                    Instruction::EditFlags(flags) => flags.exec(self),
                    Instruction::Include(include) => match include.exec(self) {
                        IncludeResult::Cached(script) => {
                            self.script_stack.push(ScriptStack {
                                script: script.clone(),
                                prev_pos: self.pos + 1,
                                prev_vars_local: std::mem::replace(
                                    &mut self.vars_local,
                                    vec![String::with_capacity(0); script.num_vars],
                                ),
                                prev_vars_match: std::mem::replace(
                                    &mut self.vars_match,
                                    vec![String::with_capacity(0); script.num_match_vars],
                                ),
                            });
                            self.pos = 0;
                            current_script = script;
                            iter = current_script.instructions.iter();
                            continue;
                        }
                        IncludeResult::Event(event) => {
                            self.pos += 1;
                            return Some(Ok(event));
                        }
                        IncludeResult::Error(err) => {
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
                        return Some(Err(RuntimeError::ScriptErrorMessage(
                            self.eval_string(&err.message).into_owned(),
                        )))
                    }
                    Instruction::Execute(execute) => {
                        self.pos += 1;
                        return Some(Ok(Event::Execute {
                            command: self.eval_string(&execute.command).into_owned(),
                            arguments: self.eval_strings_owned(&execute.arguments),
                        }));
                    }
                    Instruction::Invalid(invalid) => {
                        return Some(Err(RuntimeError::InvalidInstruction(invalid.clone())));
                    }

                    #[cfg(test)]
                    Instruction::External((command, params)) => {
                        self.pos += 1;
                        return Some(Ok(Event::TestCommand {
                            command: command.to_string(),
                            params: params
                                .iter()
                                .map(|p| self.eval_string(p).to_string())
                                .collect(),
                        }));
                    }
                }

                self.pos += 1;
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

        let global_flags = self.get_global_flags();
        if self.has_changes || !global_flags.is_empty() {
            let message_id_ = match self.build_message_id() {
                Ok(message_id_) => message_id_,
                Err(err) => return Some(Err(err)),
            };
            for action in self.actions.iter_mut() {
                if let Action::Keep { flags, message_id } = action {
                    if flags.is_empty() && !global_flags.is_empty() {
                        *flags = global_flags;
                    }
                    *message_id = message_id_;
                    break;
                }
            }
        }

        None
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
                self.envelope.push((envelope, value.into()));
            }
        }
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

    pub fn set_env_variable(&mut self, name: impl Into<String>, value: impl Into<Cow<'x, str>>) {
        self.vars_env.insert(name.into(), value.into());
    }

    pub fn with_env_variable(
        mut self,
        name: impl Into<String>,
        value: impl Into<Cow<'x, str>>,
    ) -> Self {
        self.set_env_variable(name, value);
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

    pub(crate) fn user_from_field(&self) -> String {
        if !self.user_full_name.is_empty() {
            format!("\"{}\" <{}>", self.user_full_name, self.user_address)
        } else {
            self.user_address.to_string()
        }
    }
}
