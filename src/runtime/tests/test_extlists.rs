/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{compiler::grammar::tests::test_extlists::TestValidExtList, Context};

use super::TestResult;

impl TestValidExtList {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut num_valid = 0;

        for list in &self.list_names {
            if ctx
                .runtime
                .valid_ext_lists
                .contains(&ctx.eval_value(list).to_string())
            {
                num_valid += 1;
            }
        }

        TestResult::Bool((num_valid == self.list_names.len()) ^ self.is_not)
    }
}
