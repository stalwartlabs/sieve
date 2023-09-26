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

use crate::{compiler::grammar::tests::test_exists::TestExists, Context};

use super::{mime::SubpartIterator, TestResult};

impl TestExists {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
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
