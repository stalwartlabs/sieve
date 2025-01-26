/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    compiler::{
        grammar::actions::action_set::{Modifier, Set},
        VariableType,
    },
    runtime::Variable,
    Context, Envelope, Event,
};
use std::fmt::Write;

impl Set {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut value = ctx.eval_value(&self.value);
        for modifier in &self.modifiers {
            value = modifier.apply(value.to_string().as_ref(), ctx).into();
        }

        ctx.set_variable(&self.name, value);
    }
}

impl Context<'_> {
    pub(crate) fn set_variable(&mut self, var_name: &VariableType, mut variable: Variable) {
        if variable.len() > self.runtime.max_variable_size {
            let mut new_variable = String::with_capacity(self.runtime.max_variable_size);
            for ch in variable.to_string().chars() {
                if ch.len_utf8() + new_variable.len() <= self.runtime.max_variable_size {
                    new_variable.push(ch);
                } else {
                    break;
                }
            }
            variable = new_variable.into();
        }

        match var_name {
            VariableType::Local(var_id) => {
                if let Some(var) = self.vars_local.get_mut(*var_id) {
                    *var = variable.clone();
                } else {
                    debug_assert!(false, "Non-existent local variable {var_id}");
                }
            }
            VariableType::Global(var_name) => {
                self.vars_global
                    .insert(var_name.to_string().into(), variable.clone());
            }
            VariableType::Envelope(env) => {
                self.add_set_envelope_event(*env, variable.to_string().into_owned());
            }
            _ => (),
        }
    }

    pub(crate) fn add_set_envelope_event(&mut self, envelope: Envelope, value: String) {
        let mut did_find = false;
        for (name, val) in self.envelope.iter_mut() {
            if *name == envelope {
                *val = Variable::String(value.clone().into());
                did_find = true;
                break;
            }
        }
        if !did_find {
            self.envelope
                .push((envelope, Variable::String(value.clone().into())));
        }
        self.queued_events = vec![Event::SetEnvelope { envelope, value }].into_iter();
    }

    pub(crate) fn get_variable(&self, var_name: &VariableType) -> Option<&Variable> {
        match var_name {
            VariableType::Local(var_id) => self.vars_local.get(*var_id),
            VariableType::Global(var_name) => self.vars_global.get(var_name.as_str()),
            VariableType::Envelope(env) => {
                self.envelope.iter().find_map(
                    |(name, val)| {
                        if name == env {
                            Some(val)
                        } else {
                            None
                        }
                    },
                )
            }
            _ => unreachable!(),
        }
    }
}

impl Modifier {
    pub(crate) fn apply(&self, input: &str, ctx: &Context) -> String {
        let max_len = ctx.runtime.max_variable_size;
        match self {
            Modifier::Lower => input.to_lowercase(),
            Modifier::Upper => input.to_uppercase(),
            Modifier::LowerFirst => {
                let mut result = String::with_capacity(input.len());
                for (pos, char) in input.chars().enumerate() {
                    if result.len() + char.len_utf8() <= max_len {
                        if pos != 0 {
                            result.push(char);
                        } else {
                            for char in char.to_lowercase() {
                                result.push(char);
                            }
                        }
                    } else {
                        return result;
                    }
                }
                result
            }
            Modifier::UpperFirst => {
                let mut result = String::with_capacity(input.len());
                for (pos, char) in input.chars().enumerate() {
                    if result.len() + char.len_utf8() <= max_len {
                        if pos != 0 {
                            result.push(char);
                        } else {
                            for char in char.to_uppercase() {
                                result.push(char);
                            }
                        }
                    } else {
                        return result;
                    }
                }
                result
            }
            Modifier::QuoteWildcard => {
                let mut result = String::with_capacity(input.len());
                for char in input.chars() {
                    if ['*', '\\', '?'].contains(&char) {
                        if result.len() + char.len_utf8() < max_len {
                            result.push('\\');
                            result.push(char);
                        } else {
                            return result;
                        }
                    } else if result.len() + char.len_utf8() <= max_len {
                        result.push(char);
                    } else {
                        return result;
                    }
                }
                result
            }
            Modifier::QuoteRegex => {
                let mut result = String::with_capacity(input.len());
                for char in input.chars() {
                    if [
                        '*', '\\', '?', '.', '[', ']', '(', ')', '+', '{', '}', '|', '^', '=', ':',
                        '$',
                    ]
                    .contains(&char)
                    {
                        if result.len() + char.len_utf8() < max_len {
                            result.push('\\');
                            result.push(char);
                        } else {
                            return result;
                        }
                    } else if result.len() + char.len_utf8() <= max_len {
                        result.push(char);
                    } else {
                        return result;
                    }
                }
                result
            }
            Modifier::Length => input.chars().count().to_string(),
            Modifier::EncodeUrl => {
                let mut buf = [0; 4];
                let mut result = String::with_capacity(input.len());

                for char in input.chars() {
                    if char.is_ascii_alphanumeric() || ['-', '.', '_', '~'].contains(&char) {
                        if result.len() < max_len {
                            result.push(char);
                        } else {
                            return result;
                        }
                    } else if result.len() + (char.len_utf8() * 3) <= max_len {
                        for byte in char.encode_utf8(&mut buf).as_bytes().iter() {
                            write!(result, "%{byte:02x}").ok();
                        }
                    } else {
                        return result;
                    }
                }
                result
            }
            Modifier::Replace { find, replace } => input.replace(
                ctx.eval_value(find).to_string().as_ref(),
                ctx.eval_value(replace).to_string().as_ref(),
            ),
        }
    }
}
