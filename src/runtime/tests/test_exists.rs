/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{TestResult, mime::SubpartIterator};
use crate::{Context, Sieve, bytecode::ops, runtime::RuntimeError};
use smallvec::{SmallVec, smallvec};

impl<'x> Context<'x> {
    pub(crate) fn test_exists(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestExists,
    ) -> Result<TestResult, RuntimeError> {
        let header_names = self.parse_header_names(script, test.header_names)?;
        let mut header_exists: SmallVec<[bool; 8]> = smallvec![false; header_names.len()];
        let parts = [self.part];
        let mut part_iter = SubpartIterator::new(self, &parts, test.mime_anychild);
        let mut result = false;

        while let Some((_, message_part)) = part_iter.next() {
            for (exists, header_name) in header_exists.iter_mut().zip(header_names.iter()) {
                if !*exists && message_part.headers.iter().any(|h| &h.name == header_name) {
                    *exists = true;
                }
            }

            if header_exists.iter().all(|v| *v) {
                result = true;
                break;
            }
        }

        Ok(TestResult::Bool(result ^ test.is_not))
    }
}
