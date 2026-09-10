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
    runtime::{RuntimeError, eval::ValueRef},
};

const ACTION_SET: u8 = 0;
const ACTION_ADD: u8 = 1;

impl<'x> Context<'x> {
    pub(crate) fn exec_editflags(
        &mut self,
        script: &'x Sieve<'x>,
        edit: &ops::EditFlags,
    ) -> Result<(), RuntimeError> {
        if edit.name.tag == tag::NONE || edit.name.tag == tag::VARIABLE_NONE {
            self.exec_editflags_implicit(script, edit)
        } else {
            self.exec_editflags_variable(script, edit)
        }
    }

    fn exec_editflags_implicit(
        &mut self,
        script: &'x Sieve<'x>,
        edit: &ops::EditFlags,
    ) -> Result<(), RuntimeError> {
        let mut flags = std::mem::take(&mut self.flags);

        match edit.action {
            ACTION_SET | ACTION_ADD => {
                if edit.action == ACTION_SET {
                    flags.clear();
                }
                self.tokenize_flags(script, edit.flags, |flag| {
                    for flag in flag.split_ascii_whitespace() {
                        if !flags.iter().any(|f| f.eq_ignore_ascii_case(flag)) {
                            flags.push(flag);
                        }
                    }
                    false
                })?;
            }
            _ => {
                self.tokenize_flags(script, edit.flags, |flag| {
                    for flag in flag.split_ascii_whitespace() {
                        if let Some(pos) = flags.iter().position(|f| f.eq_ignore_ascii_case(flag)) {
                            flags.swap_remove(pos);
                        }
                    }
                    false
                })?;
            }
        }

        self.flags = flags;
        Ok(())
    }

    fn exec_editflags_variable(
        &mut self,
        script: &'x Sieve<'x>,
        edit: &ops::EditFlags,
    ) -> Result<(), RuntimeError> {
        let value = match edit.action {
            ACTION_SET => {
                let mut flags = String::new();
                self.tokenize_flags(script, edit.flags, |flag| {
                    if !contains_flag(&flags, flag) {
                        if !flags.is_empty() {
                            flags.push(' ');
                        }
                        flags.push_str(flag);
                    }
                    false
                })?;
                flags
            }
            ACTION_ADD => {
                let mut new_flags = self
                    .get_variable(script, edit.name)?
                    .map(|v| v.to_string().into_owned())
                    .unwrap_or_default();

                self.tokenize_flags(script, edit.flags, |flag| {
                    if !contains_flag(&new_flags, flag) {
                        if !new_flags.is_empty() {
                            new_flags.push(' ');
                        }
                        new_flags.push_str(flag);
                    }
                    false
                })?;
                new_flags
            }
            _ => {
                let flags = self
                    .get_variable(script, edit.name)?
                    .map(|v| v.to_string().into_owned())
                    .unwrap_or_default();
                let mut current_flags: Vec<&str> = flags.split(' ').collect();
                self.tokenize_flags(script, edit.flags, |flag| {
                    if let Some(pos) = current_flags
                        .iter()
                        .position(|lflag| lflag.eq_ignore_ascii_case(flag))
                    {
                        current_flags.swap_remove(pos);
                    }
                    false
                })?;
                current_flags.join(" ")
            }
        };
        self.set_variable(script, edit.name, value.into())
    }

    pub(crate) fn tokenize_flags(
        &self,
        script: &'x Sieve<'x>,
        strings: Range,
        mut cb: impl FnMut(&'x str) -> bool,
    ) -> Result<bool, RuntimeError> {
        let mut iter = script.recs(strings)?;
        let mut pos = 0;
        while let Some(rec) = iter.next() {
            let value = ValueRef::decode(script, rec, &mut iter)?;
            let is_single = pos == 0 && iter.len() == 0;
            let flag = self.eval_value_ref(script, value)?;
            let flag = self.intern_cow(flag.into_string());
            if !flag.is_empty() {
                if is_single {
                    for flag in flag.split_ascii_whitespace() {
                        if !flag.is_empty() && cb(flag) {
                            return Ok(true);
                        }
                    }
                } else if cb(flag.trim()) {
                    return Ok(true);
                }
            }
            pos += 1;
        }
        Ok(false)
    }

    pub(crate) fn get_local_flags(
        &self,
        script: &'x Sieve<'x>,
        strings: Range,
    ) -> Result<&'x [&'x str], RuntimeError> {
        let mut flags: smallvec::SmallVec<[&'x str; 8]> = smallvec::SmallVec::new();
        self.tokenize_flags(script, strings, |flag| {
            flags.push(flag);
            false
        })?;
        Ok(self.alloc_strs(&flags))
    }

    pub(crate) fn get_global_flags(&self) -> &'x [&'x str] {
        self.alloc_strs(&self.flags)
    }

    pub(crate) fn global_flags(&self) -> &[&'x str] {
        &self.flags
    }

    pub(crate) fn get_local_or_global_flags(
        &self,
        script: &'x Sieve<'x>,
        strings: Range,
    ) -> Result<&'x [&'x str], RuntimeError> {
        if strings.is_empty() {
            Ok(self.get_global_flags())
        } else {
            self.get_local_flags(script, strings)
        }
    }
}

fn contains_flag(flags: &str, flag: &str) -> bool {
    flags.split(' ').any(|kept| kept.eq_ignore_ascii_case(flag))
}
