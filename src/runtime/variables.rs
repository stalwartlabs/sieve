/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
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
