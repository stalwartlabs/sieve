/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::sync::Arc;

use crate::{
    Context,
    compiler::{
        Value, VariableType,
        grammar::actions::action_flags::{Action, EditFlags},
    },
};

impl EditFlags {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        match &self.name {
            Some(var_name) => self.exec_variable(ctx, var_name),
            None => self.exec_implicit(ctx),
        }
    }

    fn exec_implicit(&self, ctx: &mut Context) {
        let mut flags = std::mem::take(&mut ctx.flags);

        match &self.action {
            Action::Set | Action::Add => {
                if matches!(&self.action, Action::Set) {
                    flags.clear();
                }
                ctx.tokenize_flags(&self.flags, |flag| {
                    for flag in flag.split_ascii_whitespace() {
                        if !flags.iter().any(|f| f.eq_ignore_ascii_case(flag)) {
                            flags.push(flag.into());
                        }
                    }
                    false
                });
            }
            Action::Remove => {
                ctx.tokenize_flags(&self.flags, |flag| {
                    for flag in flag.split_ascii_whitespace() {
                        if let Some(pos) = flags.iter().position(|f| f.eq_ignore_ascii_case(flag)) {
                            flags.swap_remove(pos);
                        }
                    }
                    false
                });
            }
        }

        ctx.flags = flags;
    }

    fn exec_variable(&self, ctx: &mut Context, var_name: &VariableType) {
        match &self.action {
            Action::Set => {
                let mut flags = String::new();
                ctx.tokenize_flags(&self.flags, |flag| {
                    if !contains_flag(&flags, flag) {
                        if !flags.is_empty() {
                            flags.push(' ');
                        }
                        flags.push_str(flag);
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

                ctx.tokenize_flags(&self.flags, |flag| {
                    if !contains_flag(&new_flags, flag) {
                        if !new_flags.is_empty() {
                            new_flags.push(' ');
                        }
                        new_flags.push_str(flag);
                    }
                    false
                });
                ctx.set_variable(var_name, new_flags.into());
            }
            Action::Remove => {
                let mut current_flags = Vec::new();
                let flags = ctx
                    .get_variable(var_name)
                    .map(|v| v.to_string().into_owned())
                    .unwrap_or_default();

                for flag in flags.split(' ') {
                    current_flags.push(flag);
                }
                ctx.tokenize_flags(&self.flags, |flag| {
                    if let Some(pos) = current_flags
                        .iter()
                        .position(|lflag| lflag.eq_ignore_ascii_case(flag))
                    {
                        current_flags.swap_remove(pos);
                    }
                    false
                });
                ctx.set_variable(var_name, current_flags.join(" ").into());
            }
        }
    }
}

fn contains_flag(flags: &str, flag: &str) -> bool {
    flags.split(' ').any(|kept| kept.eq_ignore_ascii_case(flag))
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
        self.flags.iter().map(|flag| flag.to_string()).collect()
    }

    pub(crate) fn global_flags(&self) -> &[Arc<str>] {
        &self.flags
    }

    pub(crate) fn get_local_or_global_flags(&self, strings: &[Value]) -> Vec<String> {
        if strings.is_empty() {
            self.get_global_flags()
        } else {
            self.get_local_flags(strings)
        }
    }
}
