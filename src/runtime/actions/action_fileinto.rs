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

use crate::{compiler::grammar::actions::action_fileinto::FileInto, Context, Event};

impl FileInto {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) {
        let folder = ctx.eval_value(&self.folder).into_string();
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
                .map(|mi| ctx.eval_value(mi).into_string()),
            special_use: self
                .special_use
                .as_ref()
                .map(|su| ctx.eval_value(su).into_string()),
            create: self.create,
            message_id: ctx.main_message_id,
        });

        ctx.queued_events = events.into_iter();
    }
}
