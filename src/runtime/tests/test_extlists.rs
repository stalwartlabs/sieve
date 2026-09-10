/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{Context, Sieve, bytecode::ops, runtime::RuntimeError};

impl<'x> Context<'x> {
    pub(crate) fn test_valid_ext_list(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestValidExtList,
    ) -> Result<TestResult, RuntimeError> {
        let all_valid = self
            .eval_values(script, test.list_names)?
            .iter()
            .all(|list| {
                self.runtime
                    .valid_ext_lists
                    .contains(list.to_string().as_ref())
            });

        Ok(TestResult::Bool(all_valid ^ test.is_not))
    }
}
