/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    Context, Sieve,
    bytecode::ops,
    runtime::{RuntimeError, handler::Action},
};

impl<'x> Context<'x> {
    pub(crate) fn exec_keep(
        &mut self,
        script: &'x Sieve<'x>,
        keep: &ops::Keep,
    ) -> Result<(), RuntimeError> {
        let created = self.build_message_id();
        let flags = self.get_local_or_global_flags(script, keep.flags)?;
        self.final_action = Some(Action::Keep {
            flags,
            message_id: self.main_message_id,
        });
        if let Some(created) = created {
            self.actions.push(created);
        }
        Ok(())
    }
}
