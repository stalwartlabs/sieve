/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{compiler::grammar::tests::test_size::TestSize, Context};

use super::TestResult;

impl TestSize {
    pub(crate) fn exec(&self, ctx: &Context) -> TestResult {
        TestResult::Bool(
            (if self.over {
                ctx.message_size > self.limit
            } else {
                ctx.message_size < self.limit
            }) ^ self.is_not,
        )
    }
}
