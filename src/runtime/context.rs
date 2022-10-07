use std::sync::Arc;

use ahash::AHashMap;
use mail_parser::Message;

use crate::{
    compiler::grammar::{instruction::Instruction, Capability},
    Context, Event, Input, Runtime, Sieve, MAX_LOCAL_VARIABLES, MAX_MATCH_VARIABLES,
};

use super::{actions::action_include::IncludeResult, tests::TestResult, RuntimeError};

pub(crate) struct ScriptStack {
    pub(crate) script: Arc<Sieve>,
    pub(crate) prev_pos: usize,
    pub(crate) prev_vars_local: Vec<String>,
    pub(crate) prev_vars_match: Vec<String>,
}

impl<'x, 'y> Context<'x, 'y> {
    pub(crate) fn new(runtime: &'y Runtime) -> Self {
        Context {
            runtime,
            raw_message: b""[..].into(),
            message: None,
            part_iter: Vec::new().into_iter(),
            part: 0,
            part_iter_stack: Vec::new(),
            pos: usize::MAX,
            test_result: false,
            script_cache: AHashMap::new(),
            script_stack: Vec::with_capacity(0),
            vars_global: AHashMap::new(),
            vars_local: Vec::with_capacity(0),
            vars_match: Vec::with_capacity(0),
            #[cfg(test)]
            test_name: String::new(),
        }
    }

    pub fn with_message(&mut self, raw_message: &'x [u8]) {
        self.raw_message = raw_message;
        self.message = Message::parse(self.raw_message).unwrap().into();
        //self.part_iter = self.message.as_ref().unwrap().html_body.clone().into_iter();
    }

    #[allow(clippy::while_let_on_iterator)]
    pub fn run(&mut self, input: Input) -> Option<Result<Event, RuntimeError>> {
        match input {
            Input::True => self.test_result = true,
            Input::False => self.test_result = false,
            Input::Script { name, script } => {
                let num_vars = script.num_vars;
                let num_match_vars = script.num_match_vars;

                if num_match_vars > MAX_MATCH_VARIABLES || num_vars > MAX_LOCAL_VARIABLES {
                    return Some(Err(RuntimeError::IllegalAction));
                }

                self.script_cache.insert(name, script.clone());
                self.script_stack.push(ScriptStack {
                    script,
                    prev_pos: self.pos,
                    prev_vars_local: std::mem::take(&mut self.vars_local),
                    prev_vars_match: std::mem::take(&mut self.vars_match),
                });
                self.pos = 0;
                self.vars_local = vec![String::with_capacity(0); num_vars];
                self.vars_match = vec![String::with_capacity(0); num_match_vars];
                self.test_result = false;
            }
        }

        let mut current_script = self.script_stack.last()?.script.clone();
        let mut iter = current_script.instructions.get(self.pos..)?.iter();

        while let Some(instruction) = iter.next() {
            //println!("{:?}", instruction);
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
                    TestResult::Event(event) => {
                        self.pos += 1;
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
                            debug_assert!(false, "Failed to clear local variables: {:?}", clear);
                        }
                    }
                    if clear.match_vars != 0 {
                        self.clear_match_variables(clear.match_vars);
                    }
                }
                Instruction::Keep(_) => {
                    println!("Test passed!");
                }
                Instruction::FileInto(_) => {
                    println!("All passed!");
                }
                Instruction::Redirect(_) => (),
                Instruction::Discard => (),
                Instruction::Stop => (),
                Instruction::Reject(_) => (),
                Instruction::ForEveryPart(_) => (),
                Instruction::Replace(_) => (),
                Instruction::Enclose(_) => (),
                Instruction::ExtractText(_) => (),
                Instruction::Convert(_) => (),
                Instruction::AddHeader(_) => (),
                Instruction::DeleteHeader(_) => (),
                Instruction::Set(set) => {
                    set.exec(self);
                }
                Instruction::Notify(_) => (),
                Instruction::Vacation(_) => (),
                Instruction::SetFlag(_) => (),
                Instruction::AddFlag(_) => (),
                Instruction::RemoveFlag(_) => (),
                Instruction::Include(include) => match include.exec(self) {
                    IncludeResult::Cached(script) => {
                        self.script_stack.push(ScriptStack {
                            script: script.clone(),
                            prev_pos: self.pos + 1,
                            prev_vars_local: std::mem::take(&mut self.vars_local),
                            prev_vars_match: std::mem::take(&mut self.vars_match),
                        });
                        self.pos = 0;
                        self.vars_local = vec![String::with_capacity(0); script.num_vars];
                        self.vars_match = vec![String::with_capacity(0); script.num_match_vars];
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
                Instruction::Return => {
                    if let Some(prev_script) = self.script_stack.pop() {
                        self.pos = prev_script.prev_pos;
                        self.vars_local = prev_script.prev_vars_local;
                        self.vars_match = prev_script.prev_vars_match;
                    }
                    current_script = self.script_stack.last()?.script.clone();
                    iter = current_script.instructions.get(self.pos..)?.iter();
                    continue;
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
                Instruction::Invalid(invalid) => {
                    return Some(Err(RuntimeError::InvalidInstruction(invalid.clone())));
                }

                #[cfg(test)]
                Instruction::TestStart(test_name) => {
                    println!("Starting test {:?}...", test_name);
                    self.test_name = test_name.clone();
                }
                #[cfg(test)]
                Instruction::TestFail(reason) => {
                    panic!(
                        "Test {} failed: {}",
                        self.test_name,
                        self.eval_string(reason)
                    );
                }
                #[cfg(test)]
                Instruction::TestSet((name, value)) => {}
            }

            self.pos += 1;
        }

        None
    }
}
