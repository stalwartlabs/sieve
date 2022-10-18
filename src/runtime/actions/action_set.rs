use crate::{
    compiler::grammar::actions::action_set::{Modifier, Set, Variable},
    Context,
};
use std::fmt::Write;

impl Set {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut value = ctx.eval_string(&self.value).into_owned();
        for modifier in &self.modifiers {
            value = modifier.apply(&value, ctx.runtime.max_variable_size);
        }

        ctx.set_variable(&self.name, value);
    }
}

impl<'x> Context<'x> {
    pub(crate) fn set_variable(&mut self, var_name: &Variable, mut variable: String) {
        if variable.len() > self.runtime.max_variable_size {
            let mut new_variable = String::with_capacity(self.runtime.max_variable_size);
            for ch in variable.chars() {
                if ch.len_utf8() + new_variable.len() <= self.runtime.max_variable_size {
                    new_variable.push(ch);
                } else {
                    break;
                }
            }
            variable = new_variable;
        }

        match var_name {
            Variable::Local(var_id) => {
                if let Some(var) = self.vars_local.get_mut(*var_id) {
                    *var = variable;
                } else {
                    debug_assert!(false, "Non-existent local variable {}", var_id);
                }
            }
            Variable::Global(var_name) => {
                self.vars_global.insert(var_name.clone(), variable);
            }
        }
    }

    pub(crate) fn get_variable(&self, var_name: &Variable) -> Option<&String> {
        match var_name {
            Variable::Local(var_id) => self.vars_local.get(*var_id),
            Variable::Global(var_name) => self.vars_global.get(var_name),
        }
    }
}

impl Modifier {
    pub(crate) fn apply(&self, input: &str, max_len: usize) -> String {
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
                            write!(result, "%{:02x}", byte).ok();
                        }
                    } else {
                        return result;
                    }
                }
                result
            }
        }
    }
}
