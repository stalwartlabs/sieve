use crate::{compiler::grammar::actions::action_fileinto::FileInto, Action, Context};

impl FileInto {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let folder = ctx.eval_string(&self.folder).into_owned();
        if ctx.has_changes {
            let bytes = ctx.build_message();
            ctx.actions.push(Action::UpdateMessage { bytes });
        }
        ctx.actions.retain(|a| match a {
            Action::Discard => false,
            Action::Keep { flags } if flags.is_empty() => false,
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
            copy: self.copy,
            create: self.create,
        });
    }
}
