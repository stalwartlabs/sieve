use mail_parser::{
    decoders::html::html_to_text, Encoding, HeaderName, Message, MessagePart, PartType,
};

use crate::{
    compiler::grammar::actions::{
        action_mime::{ExtractText, Replace},
        action_set::Variable,
    },
    runtime::tests::mime::NestedParts,
    Context,
};

use super::action_editheader::RemoveCrLf;

#[derive(Clone, Debug)]
pub(crate) struct ReplacePart<'x> {
    pub(crate) part: MessagePart<'x>,
    pub(crate) part_id: usize,
}

impl Replace {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) {
        // Remove any headers added to this part
        if ctx.header_insertions.iter().any(|h| h.part_id == ctx.part) {
            let mut new_header_insertions = Vec::with_capacity(ctx.header_deletions.len());
            let mut new_header_id = 0;
            for mut header in ctx.header_insertions.drain(..) {
                if header.part_id != ctx.part {
                    header.header.offset_field = new_header_id;
                    new_header_id += 1;
                    new_header_insertions.push(header);
                } else {
                    ctx.message_size -=
                        header.header.name.as_str().len() + header.header.value.len() + 4
                }
            }
            ctx.header_insertions = new_header_insertions;
        }

        // Mark part and subparts as deleted
        if !ctx.part_deletions.contains(&ctx.part) {
            if let Some(part) = message.parts.get(0) {
                ctx.message_size -= part.offset_end + part.offset_header;
            }
            ctx.part_deletions
                .extend(message.find_nested_parts_ids(ctx, true));
        } else {
            // Part replaced before, delete
            let mut part_size = 0;
            ctx.part_replacements.retain(|p| {
                if p.part_id != ctx.part {
                    true
                } else {
                    part_size = p.part.offset_end;
                    false
                }
            });
            ctx.message_size -= part_size;
        }

        if ctx.part == 0 {
            let mut replaced_from = false;
            let mut replaced_subject = false;

            if let Some(from) = self.from.as_ref().map(|f| ctx.eval_string(f)) {
                if !from.is_empty() {
                    ctx.insert_header(
                        0,
                        HeaderName::Other("From".into()),
                        from.as_ref().remove_crlf(),
                        false,
                    );
                    replaced_from = true;
                }
            }
            if let Some(subject) = self.subject.as_ref().map(|f| ctx.eval_string(f)) {
                if !subject.is_empty() {
                    ctx.insert_header(
                        0,
                        HeaderName::Other("Subject".into()),
                        subject.as_ref().remove_crlf(),
                        false,
                    );
                    replaced_subject = true;
                }
            }
            if let Some(subject) = message.get_subject() {
                ctx.insert_header(
                    0,
                    HeaderName::Other(
                        (if !replaced_subject {
                            "Original-Subject"
                        } else {
                            "Subject"
                        })
                        .into(),
                    ),
                    subject.to_string(),
                    false,
                );
            }
            if let Some(from) = message.get_header_raw("From") {
                ctx.insert_header(
                    0,
                    HeaderName::Other(
                        (if !replaced_from {
                            "Original-From"
                        } else {
                            "From"
                        })
                        .into(),
                    ),
                    from.remove_crlf(),
                    false,
                );
            }
            if let Some(from) = message.get_header_raw("To") {
                ctx.insert_header(0, HeaderName::Other("To".into()), from.remove_crlf(), false);
            }
        }

        if !self.mime {
            ctx.insert_header(
                ctx.part,
                HeaderName::Other("Content-type".into()),
                "text/plain; charset=utf-8".to_string(),
                false,
            );
        }

        let body = ctx.eval_string(&self.replacement).into_owned();
        let body_len = body.len();
        ctx.message_size += body_len;
        ctx.part_replacements.push(ReplacePart {
            part: MessagePart {
                headers: vec![],
                is_encoding_problem: false,
                body: PartType::Text(body.into()),
                encoding: if !self.mime {
                    Encoding::QuotedPrintable
                } else {
                    Encoding::None
                },
                offset_header: 0,
                offset_body: 0, //TODO check get part functions
                offset_end: body_len,
            },
            part_id: ctx.part,
        });
    }
}

impl ExtractText {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) {
        let mut value = String::new();

        if !ctx.part_iter_stack.is_empty() {
            match message.parts.get(ctx.part).map(|p| &p.body) {
                Some(PartType::Text(text)) => {
                    value = if let Some(first) = &self.first {
                        text.chars().take(*first).collect()
                    } else {
                        text.as_ref().to_string()
                    };
                }
                Some(PartType::Html(html)) => {
                    value = if let Some(first) = &self.first {
                        html_to_text(html.as_ref()).chars().take(*first).collect()
                    } else {
                        html_to_text(html.as_ref())
                    };
                }
                _ => (),
            }

            if !self.modifiers.is_empty() && !value.is_empty() {
                for modifier in &self.modifiers {
                    value = modifier.apply(&value);
                }
            }
        }

        match &self.name {
            Variable::Local(var_id) => {
                if let Some(var) = ctx.vars_local.get_mut(*var_id) {
                    *var = value;
                } else {
                    debug_assert!(false, "Non-existent local variable {}", var_id);
                }
            }
            Variable::Global(var_name) => {
                ctx.vars_global.insert(var_name.clone(), value);
            }
        }
    }
}

impl<'x> Context<'x> {
    #[inline(always)]
    pub(crate) fn get_part(
        &self,
        message: &'x Message<'x>,
        part_id: usize,
    ) -> Option<&MessagePart<'x>> {
        if self.part_deletions.contains(&part_id) {
            self.part_replacements.iter().find_map(|p| {
                if p.part_id == part_id {
                    Some(&p.part)
                } else {
                    None
                }
            })
        } else {
            message.parts.get(part_id)
        }
    }
}
