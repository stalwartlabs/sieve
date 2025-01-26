/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    compiler::{
        grammar::actions::action_flags::{Action, EditFlags},
        Value, VariableType,
    },
    Context,
};

impl EditFlags {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut var_name_ = None;
        let var_name = self.name.as_ref().unwrap_or_else(|| {
            var_name_.get_or_insert_with(|| VariableType::Global("__flags".to_string()))
        });

        match &self.action {
            Action::Set => {
                let mut flags_lc = Vec::new();
                let mut flags = String::new();
                ctx.tokenize_flags(&self.flags, |flag| {
                    let flag_lc = flag.to_lowercase();
                    if !flags_lc.contains(&flag_lc) {
                        if !flags.is_empty() {
                            flags.push(' ');
                        }
                        flags.push_str(flag);
                        flags_lc.push(flag_lc);
                    }
                    false
                });
                ctx.set_variable(var_name, flags.into());
            }
            Action::Add => {
                let mut new_flags = ctx
                    .get_variable(var_name)
                    .map(|v| v.to_string())
                    .unwrap_or_default()
                    .into_owned();
                let mut current_flags = new_flags
                    .split(' ')
                    .map(|f| f.to_lowercase())
                    .collect::<Vec<_>>();

                ctx.tokenize_flags(&self.flags, |flag| {
                    let flag_lc = flag.to_lowercase();
                    if !current_flags.contains(&flag_lc) {
                        if !new_flags.is_empty() {
                            new_flags.push(' ');
                        }
                        new_flags.push_str(flag);
                        current_flags.push(flag_lc);
                    }
                    false
                });
                ctx.set_variable(var_name, new_flags.into());
            }
            Action::Remove => {
                let mut current_flags = Vec::new();
                let mut current_flags_lc = Vec::new();
                let flags = ctx
                    .get_variable(var_name)
                    .map(|v| v.to_string().into_owned())
                    .unwrap_or_default();

                for flag in flags.split(' ') {
                    current_flags.push(flag);
                    current_flags_lc.push(flag.to_lowercase());
                }
                ctx.tokenize_flags(&self.flags, |flag| {
                    let flag = flag.to_lowercase();
                    if let Some(pos) = current_flags_lc.iter().position(|lflag| lflag == &flag) {
                        current_flags.swap_remove(pos);
                        current_flags_lc.swap_remove(pos);
                    }
                    false
                });
                ctx.set_variable(var_name, current_flags.join(" ").into());
            }
        }
    }
}

impl Context<'_> {
    pub(crate) fn tokenize_flags(
        &self,
        strings: &[Value],
        mut cb: impl FnMut(&str) -> bool,
    ) -> bool {
        for (pos, string) in strings.iter().enumerate() {
            let flag_ = self.eval_value(string);
            let flag = flag_.to_string();
            if !flag.is_empty() {
                if pos == 0 && strings.len() == 1 {
                    for flag in flag.split_ascii_whitespace() {
                        if !flag.is_empty() && cb(flag) {
                            return true;
                        }
                    }
                } else if cb(flag.trim()) {
                    return true;
                }
            }
        }
        false
    }

    pub(crate) fn get_local_flags(&self, strings: &[Value]) -> Vec<String> {
        let mut flags = Vec::new();
        self.tokenize_flags(strings, |flag| {
            flags.push(flag.to_string());
            false
        });
        flags
    }

    pub(crate) fn get_global_flags(&self) -> Vec<String> {
        match self.vars_global.get("__flags") {
            Some(flags) if !flags.is_empty() => flags
                .to_string()
                .split(' ')
                .map(|s| s.to_string())
                .collect::<Vec<String>>(),
            _ => Vec::new(),
        }
    }

    pub(crate) fn get_local_or_global_flags(&self, strings: &[Value]) -> Vec<String> {
        if strings.is_empty() {
            self.get_global_flags()
        } else {
            self.get_local_flags(strings)
        }
    }
}
