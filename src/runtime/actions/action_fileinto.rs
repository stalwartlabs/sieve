/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{compiler::grammar::actions::action_fileinto::FileInto, Context, Event};

impl FileInto {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let folder = ctx.eval_value(&self.folder).to_string().into_owned();
        let mut events = Vec::with_capacity(2);
        if let Some(event) = ctx.build_message_id() {
            events.push(event);
        }

        if !self.copy
            && !matches!(&ctx.final_event, Some(Event::Keep { flags, .. }) if !flags.is_empty())
        {
            ctx.final_event = None;
        }

        events.push(Event::FileInto {
            folder,
            flags: ctx.get_local_or_global_flags(&self.flags),
            mailbox_id: self
                .mailbox_id
                .as_ref()
                .map(|mi| ctx.eval_value(mi).to_string().into_owned()),
            special_use: self
                .special_use
                .as_ref()
                .map(|su| ctx.eval_value(su).to_string().into_owned()),
            create: self.create,
            message_id: ctx.main_message_id,
        });

        ctx.queued_events = events.into_iter();
    }
}
