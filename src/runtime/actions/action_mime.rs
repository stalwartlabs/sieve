/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::cmp::Reverse;

use mail_parser::{
    decoders::html::html_to_text, Encoding, HeaderName, Message, MessagePart, PartType,
};

use crate::{
    compiler::{
        grammar::actions::action_mime::{Enclose, ExtractText, Replace},
        VariableType,
    },
    Context, Event,
};

use super::action_editheader::RemoveCrLf;

#[cfg(not(test))]
use mail_builder::headers::message_id::generate_message_id_header;

impl Replace {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        // Delete children parts
        let mut part_ids = ctx.find_nested_parts_ids(false);
        part_ids.sort_unstable_by_key(|a| Reverse(*a));
        for part_id in part_ids {
            ctx.message.parts.remove(part_id as usize);
        }
        ctx.has_changes = true;

        // Update part
        let body = ctx.eval_value(&self.replacement).to_string().into_owned();
        let body_len = body.len();

        let part = &mut ctx.message.parts[ctx.part as usize];

        ctx.message_size = ctx.message_size + body_len
            - (if part.offset_body != 0 {
                (part.offset_end - part.offset_header) as usize
            } else {
                part.body.len()
            });
        part.body = PartType::Text(body.into());
        part.encoding = if !self.mime {
            Encoding::QuotedPrintable
        } else {
            Encoding::None
        };
        part.offset_body = 0;
        let prev_headers = std::mem::take(&mut part.headers);
        let mut add_date = true;

        if ctx.part == 0 {
            for mut header in prev_headers {
                let mut size = (header.offset_end - header.offset_field) as usize;
                match &header.name {
                    HeaderName::Subject => {
                        if self.subject.is_some() {
                            header.name = HeaderName::Other("Original-Subject".into());
                            header.offset_field = header.offset_start;
                            size += "Original-".len();
                        }
                    }
                    HeaderName::From => {
                        if self.from.is_some() {
                            header.name = HeaderName::Other("Original-From".into());
                            header.offset_field = header.offset_start;
                            size += "Original-".len();
                        }
                    }

                    HeaderName::To | HeaderName::Cc | HeaderName::Bcc | HeaderName::Received => (),
                    HeaderName::Date => {
                        add_date = false;
                    }
                    _ => continue,
                }
                ctx.message_size += size;
                part.headers.push(header);
            }

            // Add From
            let mut add_from = true;
            if let Some(from) = self.from.as_ref().map(|f| ctx.eval_value(f)) {
                if !from.is_empty() {
                    ctx.insert_header(
                        0,
                        HeaderName::Other("From".into()),
                        from.to_string()
                            .as_ref()
                            .remove_crlf(ctx.runtime.max_header_size),
                        true,
                    );
                    add_from = false;
                }
            }
            if add_from {
                ctx.insert_header(
                    0,
                    HeaderName::Other("From".to_string().into()),
                    ctx.user_from_field(),
                    true,
                );
            }

            // Add Subject
            if let Some(subject) = self.subject.as_ref().map(|f| ctx.eval_value(f)) {
                if !subject.is_empty() {
                    ctx.insert_header(
                        0,
                        HeaderName::Other("Subject".into()),
                        subject
                            .to_string()
                            .as_ref()
                            .remove_crlf(ctx.runtime.max_header_size),
                        true,
                    );
                }
            }

            // Add Date
            if add_date {
                #[cfg(not(test))]
                let header_value = mail_builder::headers::date::Date::now().to_rfc822();
                #[cfg(test)]
                let header_value = "Tue, 20 Nov 2022 05:14:20 -0300".to_string();

                ctx.insert_header(
                    0,
                    HeaderName::Other("Date".to_string().into()),
                    header_value,
                    true,
                );
            }

            // Add Message-ID
            let mut header_value = Vec::with_capacity(20);
            #[cfg(not(test))]
            generate_message_id_header(&mut header_value, &ctx.runtime.local_hostname).unwrap();
            #[cfg(test)]
            header_value.extend_from_slice(b"<auto-generated@message-id>");

            ctx.insert_header(
                0,
                HeaderName::Other("Message-ID".to_string().into()),
                String::from_utf8(header_value).unwrap(),
                true,
            );
        }

        if !self.mime {
            ctx.insert_header(
                ctx.part,
                HeaderName::Other("Content-Type".into()),
                "text/plain; charset=utf-8".to_string(),
                true,
            );
        }
    }
}

impl Enclose {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let body = ctx.eval_value(&self.value).to_string().into_owned();
        let subject = self
            .subject
            .as_ref()
            .map(|s| {
                ctx.eval_value(s)
                    .to_string()
                    .as_ref()
                    .remove_crlf(ctx.runtime.max_header_size)
            })
            .or_else(|| ctx.message.subject().map(|s| s.to_string()))
            .unwrap_or_default();

        let message = std::mem::take(&mut ctx.message);
        #[cfg(test)]
        let boundary = make_test_boundary();
        #[cfg(not(test))]
        let boundary = mail_builder::mime::make_boundary(".");

        ctx.message_size += ((boundary.len() + 6) * 3) + body.len() + 2;
        ctx.part = 0;
        ctx.has_changes = true;
        ctx.message = Message {
            html_body: Vec::with_capacity(0),
            text_body: Vec::with_capacity(0),
            attachments: Vec::with_capacity(0),
            parts: vec![
                MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: PartType::Multipart(vec![1, 2]),
                    encoding: Encoding::None,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
                MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: PartType::Text(body.into()),
                    encoding: Encoding::QuotedPrintable, // Flag non-mime part
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
                MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: PartType::Message(message),
                    encoding: Encoding::QuotedPrintable, // Flag non-mime part
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
            ],
            raw_message: b""[..].into(),
        };

        ctx.insert_header(
            0,
            HeaderName::Other("Content-Type".into()),
            format!("multipart/mixed; boundary=\"{boundary}\""),
            true,
        );
        ctx.insert_header(0, HeaderName::Other("Subject".into()), subject, true);
        ctx.insert_header(
            1,
            HeaderName::Other("Content-Type".into()),
            "text/plain; charset=utf-8",
            true,
        );
        ctx.insert_header(
            2,
            HeaderName::Other("Content-Type".into()),
            "message/rfc822",
            true,
        );

        let mut add_date = true;
        let mut add_message_id = true;
        let mut add_from = true;

        for header in &self.headers {
            let header = ctx.eval_value(header);
            if let Some((mut header_name, mut header_value)) =
                header.to_string().as_ref().split_once(':')
            {
                header_name = header_name.trim();
                header_value = header_value.trim();
                if !header_value.is_empty() {
                    if let Some(name) = HeaderName::parse(header_name) {
                        if !ctx.runtime.protected_headers.contains(&name) {
                            match &name {
                                HeaderName::Date => {
                                    add_date = false;
                                }
                                HeaderName::From => {
                                    add_from = false;
                                }
                                HeaderName::MessageId => {
                                    add_message_id = false;
                                }
                                _ => (),
                            }

                            ctx.insert_header(
                                0,
                                HeaderName::Other(header_name.to_string().into()),
                                header_value.remove_crlf(ctx.runtime.max_header_size),
                                true,
                            );
                        }
                    }
                }
            }
        }

        if add_from {
            ctx.insert_header(
                0,
                HeaderName::Other("From".to_string().into()),
                ctx.user_from_field(),
                true,
            );
        }

        if add_date {
            #[cfg(not(test))]
            let header_value = mail_builder::headers::date::Date::now().to_rfc822();
            #[cfg(test)]
            let header_value = "Tue, 20 Nov 2022 05:14:20 -0300".to_string();

            ctx.insert_header(
                0,
                HeaderName::Other("Date".to_string().into()),
                header_value,
                true,
            );
        }

        if add_message_id {
            let mut header_value = Vec::with_capacity(20);
            #[cfg(not(test))]
            generate_message_id_header(&mut header_value, &ctx.runtime.local_hostname).unwrap();
            #[cfg(test)]
            header_value.extend_from_slice(b"<auto-generated@message-id>");

            ctx.insert_header(
                0,
                HeaderName::Other("Message-ID".to_string().into()),
                String::from_utf8(header_value).unwrap(),
                true,
            );
        }
    }
}

impl ExtractText {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut value = String::new();

        if !ctx.part_iter_stack.is_empty() {
            match ctx.message.parts.get(ctx.part as usize).map(|p| &p.body) {
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
                    value = modifier.apply(&value, ctx);
                }
            }
        }

        match &self.name {
            VariableType::Local(var_id) => {
                if let Some(var) = ctx.vars_local.get_mut(*var_id) {
                    *var = value.into();
                } else {
                    debug_assert!(false, "Non-existent local variable {var_id}");
                }
            }
            VariableType::Global(var_name) => {
                ctx.vars_global
                    .insert(var_name.to_string().into(), value.into());
            }
            VariableType::Envelope(env) => {
                ctx.add_set_envelope_event(*env, value);
            }
            _ => (),
        }
    }
}

enum StackItem<'x> {
    Message(&'x Message<'x>),
    Boundary(&'x str),
    None,
}

impl Context<'_> {
    pub(crate) fn build_message_id(&mut self) -> Option<Event> {
        if self.has_changes {
            self.last_message_id += 1;
            self.main_message_id = self.last_message_id;
            self.has_changes = false;
            let message = self.build_message();
            Some(Event::CreatedMessage {
                message_id: self.main_message_id,
                message,
            })
        } else {
            None
        }
    }

    pub(crate) fn build_message(&mut self) -> Vec<u8> {
        let mut current_message = &self.message;
        let mut current_boundary = "";
        let mut message = Vec::with_capacity(self.message_size);
        let mut iter = [0u32].iter();
        let mut iter_stack = Vec::new();
        let mut last_offset = 0;

        'outer: loop {
            while let Some(part) = iter
                .next()
                .and_then(|p| current_message.parts.get(*p as usize))
            {
                if last_offset > 0 {
                    message.extend_from_slice(
                        &current_message.raw_message
                            [last_offset as usize..part.offset_header as usize],
                    );
                } else if !current_boundary.is_empty()
                    && part.offset_end == 0
                    && !matches!(iter_stack.last(), Some((StackItem::Message(_), _, _)))
                {
                    message.extend_from_slice(b"\r\n--");
                    message.extend_from_slice(current_boundary.as_bytes());
                    message.extend_from_slice(b"\r\n");
                }

                let mut ct_pos = usize::MAX;

                for (header_pos, header) in part.headers.iter().enumerate() {
                    if header.offset_end != 0 {
                        if header.offset_field != header.offset_start {
                            message.extend_from_slice(
                                &current_message.raw_message
                                    [header.offset_field as usize..header.offset_end as usize],
                            );
                        } else {
                            // Renamed header
                            message.extend_from_slice(header.name.as_str().as_bytes());
                            message.extend_from_slice(b":");
                            message.extend_from_slice(
                                &current_message.raw_message
                                    [header.offset_start as usize..header.offset_end as usize],
                            );
                        }
                    } else {
                        if header.name == HeaderName::Other("Content-Type".into()) {
                            ct_pos = header_pos;
                        }

                        message.extend_from_slice(header.name.as_str().as_bytes());
                        message.extend_from_slice(b": ");
                        message.extend_from_slice(header.value.as_text().unwrap_or("").as_bytes());
                        message.extend_from_slice(b"\r\n");
                    }
                }

                if part.offset_body != 0 || part.encoding != Encoding::None {
                    // Add CRLF unless this is a :mime replaced part
                    message.extend_from_slice(b"\r\n");
                }

                if part.offset_body != 0 {
                    // Original message part

                    if let PartType::Multipart(subparts) = &part.body {
                        // Multiparts contain offsets of the entire part, do not add.
                        iter_stack.push((
                            StackItem::None,
                            part,
                            std::mem::replace(&mut iter, subparts.iter()),
                        ));
                        last_offset = part.offset_body;
                        continue 'outer;
                    } else {
                        message.extend_from_slice(
                            &current_message.raw_message
                                [part.offset_body as usize..part.offset_end as usize],
                        )
                    }
                } else {
                    match &part.body {
                        PartType::Message(nested_message) => {
                            // Enclosed message
                            iter_stack.push((
                                StackItem::Message(current_message),
                                part,
                                std::mem::replace(&mut iter, [0].iter()),
                            ));
                            current_message = nested_message;
                            continue 'outer;
                        }
                        PartType::Multipart(subparts) => {
                            // Multipart enclosing nested message, obtain MIME boundary
                            let prev_boundary = std::mem::replace(
                                &mut current_boundary,
                                if ct_pos != usize::MAX {
                                    part.headers[ct_pos]
                                        .value
                                        .as_text()
                                        .and_then(|h| h.split_once("boundary=\""))
                                        .and_then(|(_, h)| h.split_once('\"'))
                                        .map(|(h, _)| h)
                                } else {
                                    None
                                }
                                .unwrap_or("invalid-boundary"),
                            );

                            // Enclose multipart
                            iter_stack.push((
                                StackItem::Boundary(prev_boundary),
                                part,
                                std::mem::replace(&mut iter, subparts.iter()),
                            ));
                            continue 'outer;
                        }
                        _ => {
                            // Replaced part
                            message.extend_from_slice(part.contents());
                        }
                    }
                }
                last_offset = part.offset_end;
            }

            if let Some((prev_item, prev_part, prev_iter)) = iter_stack.pop() {
                match prev_item {
                    StackItem::Message(prev_message) => {
                        if last_offset > 0 {
                            if let Some(bytes) =
                                current_message.raw_message.get(last_offset as usize..)
                            {
                                message.extend_from_slice(bytes);
                            }
                            last_offset = 0;
                        }
                        current_message = prev_message;
                    }
                    StackItem::Boundary(prev_boundary) => {
                        if !current_boundary.is_empty() {
                            message.extend_from_slice(b"\r\n--");
                            message.extend_from_slice(current_boundary.as_bytes());
                            message.extend_from_slice(b"--\r\n");
                        }
                        current_boundary = prev_boundary;
                    }
                    StackItem::None => {
                        message.extend_from_slice(
                            &current_message.raw_message
                                [last_offset as usize..prev_part.offset_end as usize],
                        );
                        last_offset = prev_part.offset_end;
                    }
                }
                iter = prev_iter;
            } else {
                break;
            }
        }

        if last_offset > 0 {
            if let Some(bytes) = current_message.raw_message.get(last_offset as usize..) {
                message.extend_from_slice(bytes);
            }
        }

        message
    }
}

#[cfg(test)]
thread_local!(static COUNTER: std::cell::Cell<u64>  = 0.into());

#[cfg(test)]
pub(crate) fn make_test_boundary() -> String {
    format!("boundary_{}", COUNTER.with(|c| { c.replace(c.get() + 1) }))
}

#[cfg(test)]
pub(crate) fn reset_test_boundary() {
    COUNTER.with(|c| c.replace(0));
}
