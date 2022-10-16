use std::borrow::Cow;

use crate::{compiler::lexer::string::StringItem, Context};

impl<'x> Context<'x> {
    pub(crate) fn eval_string<'z: 'y, 'y>(&'z self, string: &'y StringItem) -> Cow<'y, str> {
        match string {
            StringItem::Text(text) => text.into(),
            StringItem::LocalVariable(var_num) => {
                if let Some(data) = self.vars_local.get(*var_num) {
                    data.into()
                } else {
                    debug_assert!(false, "Failed to access local variable {}", var_num);
                    ""[..].into()
                }
            }
            StringItem::MatchVariable(var_num) => {
                if let Some(data) = self.vars_match.get(*var_num) {
                    data.into()
                } else {
                    debug_assert!(false, "Failed to access match variable {}", var_num);
                    ""[..].into()
                }
            }
            StringItem::GlobalVariable(var_name) => {
                if let Some(data) = self.vars_global.get(var_name) {
                    data.into()
                } else {
                    ""[..].into()
                }
            }
            StringItem::EnvironmentVariable(var_name) => {
                if let Some(data) = self.vars_env.get(var_name) {
                    data.as_ref().into()
                } else if let Some(data) = self.runtime.environment.get(var_name) {
                    data.as_ref().into()
                } else {
                    ""[..].into()
                }
            }
            StringItem::List(list) => {
                let mut data = String::new();
                for item in list {
                    match item {
                        StringItem::Text(string) => {
                            data.push_str(string);
                        }
                        StringItem::LocalVariable(var_num) => {
                            if let Some(string) = self.vars_local.get(*var_num) {
                                data.push_str(string);
                            } else {
                                debug_assert!(false, "Failed to access local variable {}", var_num);
                            }
                        }
                        StringItem::MatchVariable(var_num) => {
                            if let Some(string) = self.vars_match.get(*var_num) {
                                data.push_str(string);
                            } else {
                                debug_assert!(false, "Failed to access match variable {}", var_num);
                            }
                        }
                        StringItem::GlobalVariable(var_name) => {
                            if let Some(string) = self.vars_global.get(var_name) {
                                data.push_str(string);
                            }
                        }
                        StringItem::EnvironmentVariable(var_name) => {
                            if let Some(string) = self.vars_env.get(var_name) {
                                data.push_str(string);
                            } else if let Some(string) = self.runtime.environment.get(var_name) {
                                data.push_str(string);
                            }
                        }
                        _ => {
                            debug_assert!(false, "This should not have happened: {:?}", string);
                        }
                    }
                }
                data.into()
            }
        }
    }

    #[inline(always)]
    pub(crate) fn eval_strings<'z: 'y, 'y>(
        &'z self,
        strings: &'y [StringItem],
    ) -> Vec<Cow<'y, str>> {
        strings.iter().map(|s| self.eval_string(s)).collect()
    }

    #[inline(always)]
    pub(crate) fn eval_strings_owned(&self, strings: &[StringItem]) -> Vec<String> {
        strings
            .iter()
            .map(|s| self.eval_string(s).into_owned())
            .collect()
    }
}

pub(crate) trait IntoString: Sized {
    fn into_string(self) -> String;
}

impl IntoString for Vec<u8> {
    fn into_string(self) -> String {
        String::from_utf8(self)
            .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned())
    }
}
