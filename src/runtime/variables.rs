/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{RuntimeError, Variable, handler::Action};
use crate::{
    Context, Envelope, Sieve,
    bytecode::rec::{Rec, tag},
};
use std::borrow::Cow;

impl<'x> Context<'x> {
    pub(crate) fn set_variable(
        &mut self,
        script: &'x Sieve<'x>,
        name: Rec,
        mut variable: Variable<'x>,
    ) -> Result<(), RuntimeError> {
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
        self.assign_variable(script, name, variable)
    }

    pub(crate) fn assign_variable(
        &mut self,
        script: &'x Sieve<'x>,
        name: Rec,
        variable: Variable<'x>,
    ) -> Result<(), RuntimeError> {
        let variable = self.intern(variable);

        match name.tag {
            tag::VAR_LOCAL => {
                let index = self.local_base() + name.c as usize;
                if let Some(var) = self.vars_local.get_mut(index) {
                    *var = variable;
                } else {
                    debug_assert!(false, "Non-existent local variable {}", name.c);
                }
            }
            tag::VAR_GLOBAL => {
                let var_name = script.str(name.str())?;
                if let Some(value) = self.vars_global.get_mut(var_name) {
                    *value = variable;
                } else {
                    self.vars_global.insert(Cow::Borrowed(var_name), variable);
                }
            }
            tag::VAR_ENVELOPE => {
                let value = self.intern_cow(variable.into_string());
                self.set_envelope_variable(Envelope::from_code(name.b), value);
            }
            _ => (),
        }
        Ok(())
    }

    pub(crate) fn set_envelope_variable(&mut self, envelope: Envelope, value: &'x str) {
        let mut did_find = false;
        for (name, val) in self.envelope.iter_mut() {
            if *name == envelope {
                *val = Variable::borrowed(value);
                did_find = true;
                break;
            }
        }
        if !did_find {
            self.envelope.push((envelope, Variable::borrowed(value)));
        }
        self.actions.push(Action::SetEnvelope { envelope, value });
    }

    pub(crate) fn get_variable(
        &self,
        script: &'x Sieve<'x>,
        name: Rec,
    ) -> Result<Option<Variable<'x>>, RuntimeError> {
        Ok(match name.tag {
            tag::VAR_LOCAL => self
                .vars_local
                .get(self.local_base() + name.c as usize)
                .cloned(),
            tag::VAR_MATCH => self
                .vars_match
                .get(self.match_base() + name.b as usize)
                .cloned(),
            tag::VAR_GLOBAL => self.vars_global.get(script.str(name.str())?).cloned(),
            tag::NONE | tag::VARIABLE_NONE => None,
            _ => Some(self.eval_value(script, name)?),
        })
    }

    pub(crate) fn set_match_variables(&mut self, set_vars: Vec<(usize, String)>) {
        let base = self.match_base();
        for (var_num, value) in set_vars {
            let value = self.alloc_str(&value);
            if let Some(var) = self.vars_match.get_mut(base + var_num) {
                *var = Variable::borrowed(value);
            } else {
                debug_assert!(false, "Invalid match variable {var_num}");
            }
        }
    }

    pub(crate) fn clear_match_variables(&mut self, mut positions: u64) {
        let base = self.match_base();
        while positions != 0 {
            let index = 63 - positions.leading_zeros();
            positions ^= 1 << index;
            if let Some(match_var) = self.vars_match.get_mut(base + index as usize) {
                if !match_var.is_empty() {
                    *match_var = Variable::default();
                }
            } else {
                debug_assert!(false, "Failed to clear match variable at index {index}.");
            }
        }
    }
}
