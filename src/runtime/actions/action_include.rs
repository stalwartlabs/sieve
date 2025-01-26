/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::sync::Arc;

use crate::{
    compiler::grammar::actions::action_include::{Include, Location},
    runtime::RuntimeError,
    Context, Event, Script, Sieve,
};

pub(crate) enum IncludeResult {
    Cached(Arc<Sieve>),
    Event(Event),
    Error(RuntimeError),
    None,
}

impl Include {
    pub(crate) fn exec(&self, ctx: &Context) -> IncludeResult {
        let script_name = ctx.eval_value(&self.value);
        if !script_name.is_empty() {
            let script_name = if self.location == Location::Global {
                Script::Global(script_name.to_string().into_owned())
            } else {
                Script::Personal(script_name.to_string().into_owned())
            };

            let cached_script = ctx.script_cache.get(&script_name);
            if !self.once || cached_script.is_none() {
                if ctx.script_stack.len() < ctx.runtime.max_nested_includes {
                    if let Some(script) = cached_script
                        .or_else(|| ctx.runtime.include_scripts.get(script_name.as_str()))
                    {
                        return IncludeResult::Cached(script.clone());
                    } else {
                        return IncludeResult::Event(Event::IncludeScript {
                            name: script_name,
                            optional: self.optional,
                        });
                    }
                } else {
                    return IncludeResult::Error(RuntimeError::TooManyIncludes);
                }
            }
        }

        IncludeResult::None
    }
}
