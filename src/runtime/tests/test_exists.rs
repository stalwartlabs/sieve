/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{compiler::grammar::tests::test_exists::TestExists, Context};

use super::{mime::SubpartIterator, TestResult};

impl TestExists {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let header_names = ctx.parse_header_names(&self.header_names);
        let mut header_exists = vec![false; header_names.len()];
        let parts = [ctx.part];
        let mut part_iter = SubpartIterator::new(ctx, &parts, self.mime_anychild);
        let mut result = false;

        while let Some((_, message_part)) = part_iter.next() {
            for (pos, header_name) in header_names.iter().enumerate() {
                if !header_exists[pos]
                    && message_part.headers.iter().any(|h| &h.name == header_name)
                {
                    header_exists[pos] = true;
                }
            }

            if header_exists.iter().all(|v| *v) {
                result = true;
                break;
            }
        }

        TestResult::Bool(result ^ self.is_not)
    }
}
