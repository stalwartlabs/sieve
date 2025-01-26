/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::Context;

use super::Variable;

impl Context<'_> {
    pub(crate) fn set_match_variables(&mut self, set_vars: Vec<(usize, String)>) {
        for (var_num, value) in set_vars {
            if let Some(var) = self.vars_match.get_mut(var_num) {
                *var = value.into();
            } else {
                debug_assert!(false, "Invalid match variable {var_num}");
            }
        }
    }

    pub(crate) fn clear_match_variables(&mut self, mut positions: u64) {
        while positions != 0 {
            let index = 63 - positions.leading_zeros();
            positions ^= 1 << index;
            if let Some(match_var) = self.vars_match.get_mut(index as usize) {
                if !match_var.is_empty() {
                    *match_var = Variable::default();
                }
            } else {
                debug_assert!(false, "Failed to clear match variable at index {index}.");
            }
        }
    }
}
