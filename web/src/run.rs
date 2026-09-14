/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use std::borrow::Cow;

use serde::Deserialize;
use sieve::{Arena, Compiler, Sieve, Status, runtime::RuntimeError};

use crate::{
    functions,
    handler::Recorder,
    output::{CompileOutput, Diagnostic, RunOutput},
    settings::{KeyValue, Settings},
};

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Request {
    pub scripts: Vec<ScriptSource>,
    pub message: String,
    pub settings: Settings,
    pub seen_ids: Vec<String>,
    pub now: i64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default)]
pub struct ScriptSource {
    pub name: String,
    pub source: String,
}

impl Request {
    fn compile_all(&self, compiler: &Compiler) -> (Vec<Sieve<'static>>, Vec<Diagnostic>) {
        let mut compiled = Vec::with_capacity(self.scripts.len());
        let mut diagnostics = Vec::new();
        for (index, script) in self.scripts.iter().enumerate() {
            match compiler.compile(to_crlf(&script.source).as_bytes()) {
                Ok(sieve) => compiled.push(sieve),
                Err(err) => diagnostics.push(Diagnostic::compile(index, &err)),
            }
        }
        (compiled, diagnostics)
    }

    pub fn compile(&self) -> CompileOutput {
        let compiler = self.settings.compiler(&mut functions::register());
        CompileOutput {
            diagnostics: self.compile_all(&compiler).1,
        }
    }

    pub fn run(&self) -> RunOutput {
        let mut functions = functions::register();
        let compiler = self.settings.compiler(&mut functions);
        let (compiled, diagnostics) = self.compile_all(&compiler);
        let mut compiled = compiled.into_iter();
        let Some(main) = compiled.next().filter(|_| diagnostics.is_empty()) else {
            return RunOutput {
                diagnostics,
                ..Default::default()
            };
        };

        let mut runtime = self.settings.runtime(&mut functions);
        for (script, sieve) in self.scripts.iter().skip(1).zip(compiled) {
            runtime.set_include_script(script.name.trim(), sieve);
        }

        let raw = to_crlf(&self.message);
        let mut arena = Arena::new();
        let mut ctx = runtime.filter(raw.as_bytes(), &main, &mut arena);
        self.settings.apply(&mut ctx, self.now);

        let mut recorder = Recorder::new(&self.settings, &self.seen_ids);
        let mut error = None;
        loop {
            match ctx.run(&mut recorder) {
                Ok(Status::Finished) => break,
                Ok(Status::Pending) => ctx.resume(false),
                Err(err) if error.is_none() => {
                    recorder.after_error = true;
                    error = Some(self.runtime_diagnostic(&err));
                }
                Err(_) => break,
            }
        }

        let mut global_variables: Vec<KeyValue> = ctx
            .global_variable_names()
            .filter_map(|name| {
                ctx.global_variable(name).map(|value| KeyValue {
                    name: name.to_string(),
                    value: value.to_string().into_owned(),
                })
            })
            .collect();
        global_variables.sort_unstable_by(|a, b| a.name.cmp(&b.name));

        let error_free = error.is_none();
        RunOutput {
            diagnostics,
            error,
            message_changed: ctx.has_message_changed(),
            instructions: ctx.instructions_executed(),
            global_variables,
            events: recorder.events,
            messages: recorder.messages,
            duplicate_ids: if error_free {
                let mut ids = self.seen_ids.clone();
                ids.extend(recorder.new_ids);
                ids.sort_unstable();
                ids.dedup();
                ids
            } else {
                self.seen_ids.clone()
            },
        }
    }

    fn runtime_diagnostic(&self, err: &RuntimeError) -> Diagnostic {
        let mut diagnostic = Diagnostic::runtime(err);
        if let RuntimeError::InvalidInstruction { name, line_num, .. } = err {
            let contains_instruction = |source: &str| {
                source
                    .lines()
                    .nth((*line_num as usize).saturating_sub(1))
                    .is_some_and(|line| {
                        line.to_ascii_lowercase()
                            .contains(&name.to_ascii_lowercase())
                    })
            };
            match self
                .scripts
                .iter()
                .position(|script| contains_instruction(&script.source))
            {
                Some(index) => diagnostic.script = index,
                None => {
                    diagnostic.line = 0;
                    diagnostic.column = 0;
                }
            }
        }
        diagnostic
    }
}

pub fn to_crlf(text: &str) -> Cow<'_, str> {
    let bytes = text.as_bytes();
    let needs_fix = bytes
        .iter()
        .enumerate()
        .any(|(pos, &b)| b == b'\n' && (pos == 0 || bytes.get(pos - 1) != Some(&b'\r')));
    if !needs_fix {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len() + text.len() / 16);
    let mut prev = '\0';
    for ch in text.chars() {
        if ch == '\n' && prev != '\r' {
            out.push('\r');
        }
        out.push(ch);
        prev = ch;
    }
    Cow::Owned(out)
}
