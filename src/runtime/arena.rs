/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use bumpalo::Bump;

#[derive(Default)]
pub struct Arena {
    pub(crate) bump: Bump,
}

impl Arena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_capacity(bytes: usize) -> Self {
        Arena {
            bump: Bump::with_capacity(bytes),
        }
    }

    pub fn reset(&mut self) {
        self.bump.reset();
    }

    pub fn allocated_bytes(&self) -> usize {
        self.bump.allocated_bytes()
    }
}
