/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{Context, bytecode::ops};

impl Context<'_> {
    pub(crate) fn test_size(&self, test: &ops::TestSize) -> TestResult {
        let size = self.message_size as u64;
        let result = if test.over {
            size > test.limit
        } else {
            size < test.limit
        };
        TestResult::Bool(result ^ test.is_not)
    }
}
