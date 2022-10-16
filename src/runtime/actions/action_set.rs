use crate::{
    compiler::grammar::actions::action_set::{Modifier, Set, Variable},
    Context,
};
use std::fmt::Write;

impl Set {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut value = ctx.eval_string(&self.value).into_owned();
        for modifier in &self.modifiers {
            value = modifier.apply(&value);
        }

        ctx.set_variable(&self.name, value);
    }
}

impl<'x> Context<'x> {
    pub(crate) fn set_variable(&mut self, var_name: &Variable, variable: String) {
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
    pub(crate) fn apply(&self, input: &str) -> String {
        match self {
            Modifier::Lower => input.to_lowercase(),
            Modifier::Upper => input.to_uppercase(),
            Modifier::LowerFirst => {
                let mut result = String::with_capacity(input.len());
                for (pos, char) in input.chars().enumerate() {
                    if pos != 0 {
                        result.push(char);
                    } else {
                        for char in char.to_lowercase() {
                            result.push(char);
                        }
                    }
                }
                result
            }
            Modifier::UpperFirst => {
                let mut result = String::with_capacity(input.len());
                for (pos, char) in input.chars().enumerate() {
                    if pos != 0 {
                        result.push(char);
                    } else {
                        for char in char.to_uppercase() {
                            result.push(char);
                        }
                    }
                }
                result
            }
            Modifier::QuoteWildcard => {
                let mut result = String::with_capacity(input.len());
                for char in input.chars() {
                    if ['*', '\\', '?'].contains(&char) {
                        result.push('\\');
                    }
                    result.push(char);
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
                        result.push('\\');
                    }
                    result.push(char);
                }
                result
            }
            Modifier::Length => input.chars().count().to_string(),
            Modifier::EncodeUrl => {
                let mut result = String::with_capacity(input.len());
                for char in input.as_bytes() {
                    if char.is_ascii_alphanumeric() || [b'-', b'.', b'_', b'~'].contains(char) {
                        result.push(char::from(*char));
                    } else {
                        write!(result, "%{:02x}", char).ok();
                    }
                }
                result
            }
        }
    }
}
