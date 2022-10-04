use crate::{compiler::lexer::string::StringItem, Context};

use super::RuntimeError;

impl<'x, 'y> Context<'x, 'y> {
    pub(crate) fn eval_string_bytes(&self, string: &StringItem) -> Vec<u8> {
        match string {
            StringItem::Text(text) => text.clone(),
            StringItem::LocalVariable(_) => todo!(),
            StringItem::MatchVariable(_) => todo!(),
            StringItem::GlobalVariable(_) => todo!(),
            StringItem::MatchMany(_) => todo!(),
            StringItem::MatchOne => todo!(),
            StringItem::List(_) => todo!(),
        }
    }

    pub(crate) fn eval_string(&self, string: &StringItem) -> Result<String, RuntimeError> {
        String::from_utf8(self.eval_string_bytes(string))
            .map_err(|err| RuntimeError::InvalidUtf8(err.into_bytes()))
    }

    pub(crate) fn eval_strings(&self, strings: &[StringItem]) -> Result<Vec<String>, RuntimeError> {
        let mut result = Vec::with_capacity(strings.len());
        for string in strings {
            result.push(self.eval_string(string)?);
        }
        Ok(result)
    }
}
