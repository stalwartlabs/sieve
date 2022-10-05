use crate::{
    compiler::grammar::actions::action_set::{Modifier, Set},
    Context,
};
use std::fmt::Write;

impl Set {
    pub(crate) fn exec(&self, ctx: &Context) -> String {
        let mut value = ctx.eval_string(&self.value).into_owned();
        if !value.is_empty() {
            for modifier in &self.modifiers {
                value = modifier.apply(&value);
            }
        }
        value
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
                        for char in char.to_lowercase() {
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
            Modifier::Length => input.len().to_string(),
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
