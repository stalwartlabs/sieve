/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    Context, Sieve,
    bytecode::{
        ops,
        rec::{Range, tag},
    },
    compiler::grammar::actions::action_set::{
        MODIFIER_ENCODE_URL, MODIFIER_LENGTH, MODIFIER_LOWER, MODIFIER_LOWER_FIRST,
        MODIFIER_QUOTE_REGEX, MODIFIER_QUOTE_WILDCARD, MODIFIER_REPLACE, MODIFIER_UPPER,
        MODIFIER_UPPER_FIRST,
    },
    runtime::{RuntimeError, Variable},
};
use std::{borrow::Cow, fmt::Write};

impl<'x> Context<'x> {
    pub(crate) fn exec_set(
        &mut self,
        script: &'x Sieve<'x>,
        set: &ops::Set,
    ) -> Result<(), RuntimeError> {
        let mut value = self.eval_value(script, set.value)?;
        if !set.modifiers.is_empty() {
            value = self.apply_modifiers(script, set.modifiers, value)?;
        }
        self.set_variable(script, set.name, value)
    }

    pub(crate) fn apply_modifiers(
        &self,
        script: &'x Sieve<'x>,
        modifiers: Range,
        mut value: Variable<'x>,
    ) -> Result<Variable<'x>, RuntimeError> {
        let mut iter = script.recs(modifiers)?;
        while let Some(modifier) = iter.next() {
            if modifier.tag != tag::MODIFIER {
                return Err(RuntimeError::InvalidBytecode);
            }
            let input = value.to_string();
            let output = match modifier.b {
                MODIFIER_REPLACE => {
                    let find = iter.next().ok_or(RuntimeError::InvalidBytecode)?;
                    let replace = iter.next().ok_or(RuntimeError::InvalidBytecode)?;
                    input.replace(
                        self.eval_value(script, find)?.to_string().as_ref(),
                        self.eval_value(script, replace)?.to_string().as_ref(),
                    )
                }
                kind => self.apply_modifier(kind, input.as_ref()),
            };
            value = Variable::String(Cow::Owned(output));
        }
        Ok(value)
    }

    pub(crate) fn apply_modifier(&self, kind: u8, input: &str) -> String {
        let max_len = self.runtime.max_variable_size;
        match kind {
            MODIFIER_LOWER => input.to_lowercase(),
            MODIFIER_UPPER => input.to_uppercase(),
            MODIFIER_LOWER_FIRST => {
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
            MODIFIER_UPPER_FIRST => {
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
            MODIFIER_QUOTE_WILDCARD => {
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
            MODIFIER_QUOTE_REGEX => {
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
            MODIFIER_LENGTH => input.chars().count().to_string(),
            MODIFIER_ENCODE_URL => {
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
            _ => input.to_string(),
        }
    }
}
