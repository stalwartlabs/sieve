/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
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
                Script::Global(script_name.into_string())
            } else {
                Script::Personal(script_name.into_string())
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
