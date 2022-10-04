use std::sync::Arc;

use ahash::AHashMap;
use mail_parser::Message;

use crate::{
    compiler::grammar::{actions::action_include::Location, command::Command, Capability},
    Context, Event, Input, Runtime, Script, Sieve,
};

use super::{test::TestResult, RuntimeError};

pub(crate) struct ScriptStack {
    script: Arc<Sieve>,
    prev_pos: usize,
    prev_vars_local: Vec<Vec<u8>>,
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
                self.script_cache.insert(name, script.clone());
                self.script_stack.push(ScriptStack {
                    script,
                    prev_pos: self.pos,
                    prev_vars_local: std::mem::take(&mut self.vars_local),
                });
                self.pos = 0;
                self.vars_local = vec![Vec::with_capacity(0); num_vars];
                self.test_result = false;
            }
        }

        let mut current_script = self.script_stack.last()?;
        let mut iter = current_script.script.commands.get(self.pos..)?.iter();

        while let Some(command) = iter.next() {
            match command {
                Command::Jz(jmp_pos) => {
                    if !self.test_result {
                        debug_assert!(*jmp_pos > self.pos);
                        self.pos = *jmp_pos;
                        iter = current_script.script.commands.get(self.pos..)?.iter();
                        continue;
                    }
                }
                Command::Jnz(jmp_pos) => {
                    if self.test_result {
                        debug_assert!(*jmp_pos > self.pos);
                        self.pos = *jmp_pos;
                        iter = current_script.script.commands.get(self.pos..)?.iter();
                        continue;
                    }
                }
                Command::Jmp(jmp_pos) => {
                    debug_assert_ne!(*jmp_pos, self.pos);
                    self.pos = *jmp_pos;
                    iter = current_script.script.commands.get(self.pos..)?.iter();
                    continue;
                }
                Command::Test(test) => match self.eval_test(test) {
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
                Command::Keep(_) => {
                    println!("Test passed!");
                }
                Command::FileInto(_) => {
                    println!("All passed!");
                }
                Command::Redirect(_) => (),
                Command::Discard => (),
                Command::Stop => (),
                Command::ForEveryPart(_) => (),
                Command::Replace(_) => (),
                Command::Enclose(_) => (),
                Command::ExtractText(_) => (),
                Command::Convert(_) => (),
                Command::AddHeader(_) => (),
                Command::DeleteHeader(_) => (),
                Command::Set(_) => (),
                Command::Notify(_) => (),
                Command::Reject(_) => (),
                Command::Vacation(_) => (),
                Command::SetFlag(_) => (),
                Command::AddFlag(_) => (),
                Command::RemoveFlag(_) => (),
                Command::Include(include) => match self.eval_string(&include.value) {
                    Ok(script_name) => {
                        if !script_name.is_empty() {
                            let script_name = if include.location == Location::Global {
                                Script::Global(script_name)
                            } else {
                                Script::Personal(script_name)
                            };

                            let cached_script = self.script_cache.get(&script_name);
                            if !include.once || cached_script.is_none() {
                                if self.script_stack.len() < self.runtime.max_include_scripts {
                                    if let Some(script) = cached_script.or_else(|| {
                                        self.runtime.include_scripts.get(script_name.as_str())
                                    }) {
                                        let num_vars = script.num_vars;
                                        self.script_stack.push(ScriptStack {
                                            script: script.clone(),
                                            prev_pos: self.pos + 1,
                                            prev_vars_local: std::mem::take(&mut self.vars_local),
                                        });
                                        self.pos = 0;
                                        self.vars_local = vec![Vec::with_capacity(0); num_vars];
                                        current_script = self.script_stack.last()?;
                                        iter = current_script.script.commands.iter();
                                        continue;
                                    } else {
                                        self.pos += 1;
                                        return Some(Ok(Event::IncludeScript {
                                            name: script_name,
                                        }));
                                    }
                                } else {
                                    return Some(Err(RuntimeError::TooManyIncludes));
                                }
                            }
                        }
                    }
                    Err(err) => return Some(Err(err)),
                },
                Command::Return => {
                    if let Some(prev_script) = self.script_stack.pop() {
                        self.pos = prev_script.prev_pos;
                        self.vars_local = prev_script.prev_vars_local;
                    }
                    current_script = self.script_stack.last()?;
                    iter = current_script.script.commands.get(self.pos..)?.iter();
                    continue;
                }
                Command::Require(capabilities) => {
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
                Command::Error(err) => {
                    return Some(Err(match self.eval_string(&err.message) {
                        Ok(message) => RuntimeError::ScriptErrorMessage(message),
                        Err(err) => err,
                    }))
                }
                Command::Invalid(invalid) => {
                    return Some(Err(RuntimeError::InvalidInstruction(invalid.clone())));
                }
            }

            self.pos += 1;
        }

        None
    }
}
