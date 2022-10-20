/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
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

use crate::{
    compiler::grammar::actions::action_fileinto::FileInto, runtime::RuntimeError, Action, Context,
};

impl FileInto {
    pub(crate) fn exec(&self, ctx: &mut Context) -> Result<(), RuntimeError> {
        let folder = ctx.eval_string(&self.folder).into_owned();
        let message_id = ctx.build_message_id()?;
        ctx.actions.retain(|a| match a {
            Action::Discard if !self.copy => false,
            Action::Keep { flags, .. } if !self.copy && flags.is_empty() => false,
            Action::FileInto {
                folder: folder_, ..
            } if folder_ == &folder => false,
            _ => true,
        });
        ctx.actions.push(Action::FileInto {
            folder,
            flags: ctx.get_local_or_global_flags(&self.flags),
            mailbox_id: self
                .mailbox_id
                .as_ref()
                .map(|mi| ctx.eval_string(mi).into_owned()),
            special_use: self
                .special_use
                .as_ref()
                .map(|su| ctx.eval_string(su).into_owned()),
            create: self.create,
            message_id,
        });
        Ok(())
    }
}
