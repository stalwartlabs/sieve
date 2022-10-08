use crate::Context;

impl<'x> Context<'x> {
    pub(crate) fn set_match_variables(&mut self, set_vars: Vec<(usize, String)>) {
        for (var_num, value) in set_vars {
            if let Some(var) = self.vars_match.get_mut(var_num) {
                *var = value;
            } else {
                debug_assert!(false, "Invalid match varialbe {}", var_num);
            }
        }
    }

    pub(crate) fn clear_match_variables(&mut self, mut positions: u64) {
        while positions != 0 {
            let index = 63 - positions.leading_zeros();
            positions ^= 1 << index;
            if let Some(match_var) = self.vars_match.get_mut(index as usize) {
                if !match_var.is_empty() {
                    *match_var = String::with_capacity(0);
                }
            } else {
                debug_assert!(false, "Failed to clear match variable at index {}.", index);
            }
        }
    }
}
