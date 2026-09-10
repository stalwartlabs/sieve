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
    pub(crate) fn exec_fileinto(
        &mut self,
        script: &'x Sieve<'x>,
        fileinto: &ops::FileInto,
    ) -> Result<(), RuntimeError> {
        let folder = self.eval_str(script, fileinto.folder)?;
        if let Some(created) = self.build_message_id() {
            self.actions.push(created);
        }

        if !fileinto.copy
            && !matches!(&self.final_action, Some(Action::Keep { flags, .. }) if !flags.is_empty())
        {
            self.final_action = None;
        }

        let action = Action::FileInto {
            folder,
            flags: self.get_local_or_global_flags(script, fileinto.flags)?,
            mailbox_id: self.eval_opt_str(script, fileinto.mailbox_id)?,
            special_use: self.eval_opt_str(script, fileinto.special_use)?,
            create: fileinto.create,
            message_id: self.main_message_id,
        };
        self.actions.push(action);
        Ok(())
    }
}
