/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use fancy_regex::{Error, Regex, RegexBuilder};

const SIZE_LIMIT: usize = 1 << 20;
const BACKTRACK_LIMIT: usize = 1_000_000;

pub(crate) fn build(pattern: &str) -> Result<Regex, Error> {
    RegexBuilder::new(pattern)
        .backtrack_limit(BACKTRACK_LIMIT)
        .delegate_size_limit(SIZE_LIMIT)
        .delegate_dfa_size_limit(SIZE_LIMIT)
        .build()
}

pub(crate) fn compile(pattern: &str) -> Option<Regex> {
    build(pattern).ok()
}
