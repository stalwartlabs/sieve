/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::action_editheader::RemoveCrLf;
use crate::{
    Context, Sieve,
    bytecode::{ops, rec::tag},
    runtime::{RuntimeError, eval::ValueRef, handler::Action},
};
use mail_parser::{
    Encoding, HeaderName, Message, MessagePart, PartType, decoders::html::html_to_text,
};
use std::cmp::Reverse;

#[cfg(not(test))]
use crate::runtime::platform;
#[cfg(not(test))]
use mail_builder::headers::date::Date;

impl<'x> Context<'x> {
    pub(crate) fn exec_replace(
        &mut self,
        script: &'x Sieve<'x>,
        replace: &ops::Replace,
    ) -> Result<(), RuntimeError> {
        let mut part_ids = self.find_nested_parts_ids(false);
        part_ids.sort_unstable_by_key(|a| Reverse(*a));
        for part_id in part_ids {
            self.message.parts.remove(part_id as usize);
        }
        self.has_changes = true;

        let body = self
            .eval_value(script, replace.replacement)?
            .into_string()
            .into_owned();
        let body_len = body.len();
        let has_subject = replace.subject.tag != tag::NONE;
        let has_from = replace.from.tag != tag::NONE;

        let part = &mut self.message.parts[self.part as usize];

        self.message_size = self.message_size + body_len
            - (if part.offset_body != 0 {
                (part.offset_end - part.offset_header) as usize
            } else {
                part.body.len()
            });
        part.body = PartType::Text(body.into());
        part.encoding = if !replace.mime {
            Encoding::QuotedPrintable
        } else {
            Encoding::None
        };
        part.offset_body = 0;
        let prev_headers = std::mem::take(&mut part.headers);
        let mut add_date = true;
        let mut has_original_from = false;

        if self.part == 0 {
            for mut header in prev_headers {
                let mut size = (header.offset_end - header.offset_field) as usize;
                match &header.name {
                    HeaderName::Subject => {
                        if has_subject {
                            header.name = HeaderName::Other("Original-Subject".into());
                            header.offset_field = header.offset_start;
                            size += "Original-".len();
                        }
                    }
                    HeaderName::From => {
                        if has_from {
                            header.name = HeaderName::Other("Original-From".into());
                            header.offset_field = header.offset_start;
                            size += "Original-".len();
                        } else {
                            has_original_from = true;
                        }
                    }

                    HeaderName::To | HeaderName::Cc | HeaderName::Bcc | HeaderName::Received => (),
                    HeaderName::Date => {
                        add_date = false;
                    }
                    _ => continue,
                }
                self.message_size += size;
                part.headers.push(header);
            }

            let mut add_from = true;
            if let Some(from) = self.eval_opt(script, replace.from)?
                && !from.is_empty()
            {
                let from = from
                    .to_string()
                    .as_ref()
                    .remove_crlf(self.runtime.max_header_size);
                self.insert_header(0, HeaderName::Other("From".into()), from, true);
                add_from = false;
            }
            if add_from && !has_original_from {
                let from = self.user_from_field();
                self.insert_header(0, HeaderName::Other("From".into()), from, true);
            }

            if let Some(subject) = self.eval_opt(script, replace.subject)?
                && !subject.is_empty()
            {
                let subject = subject
                    .to_string()
                    .as_ref()
                    .remove_crlf(self.runtime.max_header_size);
                self.insert_header(0, HeaderName::Other("Subject".into()), subject, true);
            }

            if add_date {
                #[cfg(not(test))]
                let header_value = Date::new(self.current_time).to_rfc822();
                #[cfg(test)]
                let header_value = "Tue, 20 Nov 2022 05:14:20 -0300".to_string();

                self.insert_header(0, HeaderName::Other("Date".into()), header_value, true);
            }

            let header_value = self.generate_message_id();
            self.insert_header(
                0,
                HeaderName::Other("Message-ID".into()),
                header_value,
                true,
            );
        }

        if !replace.mime {
            self.insert_header(
                self.part,
                HeaderName::Other("Content-Type".into()),
                "text/plain; charset=utf-8",
                true,
            );
        }

        Ok(())
    }

    pub(crate) fn exec_enclose(
        &mut self,
        script: &'x Sieve<'x>,
        enclose: &ops::Enclose,
    ) -> Result<(), RuntimeError> {
        let body = self
            .eval_value(script, enclose.value)?
            .into_string()
            .into_owned();
        let subject = match self.eval_opt(script, enclose.subject)? {
            Some(subject) => subject
                .to_string()
                .as_ref()
                .remove_crlf(self.runtime.max_header_size),
            None => self
                .message
                .subject()
                .map(|s| s.to_string())
                .unwrap_or_default(),
        };

        let message = std::mem::take(&mut self.message);
        #[cfg(test)]
        let boundary = make_test_boundary();
        #[cfg(not(test))]
        let boundary = platform::make_boundary();

        self.message_size += ((boundary.len() + 6) * 3) + body.len() + 2;
        self.part = 0;
        self.has_changes = true;
        self.message = Message {
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
                    encoding: Encoding::QuotedPrintable,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
                MessagePart {
                    headers: vec![],
                    is_encoding_problem: false,
                    body: PartType::Message(message),
                    encoding: Encoding::QuotedPrintable,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
            ],
            raw_message: b""[..].into(),
        };

        self.insert_header(
            0,
            HeaderName::Other("Content-Type".into()),
            format!("multipart/mixed; boundary=\"{boundary}\""),
            true,
        );
        self.insert_header(0, HeaderName::Other("Subject".into()), subject, true);
        self.insert_header(
            1,
            HeaderName::Other("Content-Type".into()),
            "text/plain; charset=utf-8",
            true,
        );
        self.insert_header(
            2,
            HeaderName::Other("Content-Type".into()),
            "message/rfc822",
            true,
        );

        let mut add_date = true;
        let mut add_message_id = true;
        let mut add_from = true;

        let mut iter = script.recs(enclose.headers)?;
        while let Some(rec) = iter.next() {
            let header = ValueRef::decode(script, rec, &mut iter)?;
            let header = self.eval_value_ref(script, header)?;
            if let Some((mut header_name, mut header_value)) =
                header.to_string().as_ref().split_once(':')
            {
                header_name = header_name.trim();
                header_value = header_value.trim();
                if !header_value.is_empty()
                    && let Some(name) = HeaderName::parse(header_name)
                    && !self.runtime.protected_headers.contains(&name)
                {
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

                    let header_value = header_value.remove_crlf(self.runtime.max_header_size);
                    self.insert_header(
                        0,
                        HeaderName::Other(header_name.to_string().into()),
                        header_value,
                        true,
                    );
                }
            }
        }

        if add_from {
            let from = self.user_from_field();
            self.insert_header(0, HeaderName::Other("From".into()), from, true);
        }

        if add_date {
            #[cfg(not(test))]
            let header_value = Date::new(self.current_time).to_rfc822();
            #[cfg(test)]
            let header_value = "Tue, 20 Nov 2022 05:14:20 -0300".to_string();

            self.insert_header(0, HeaderName::Other("Date".into()), header_value, true);
        }

        if add_message_id {
            let header_value = self.generate_message_id();
            self.insert_header(
                0,
                HeaderName::Other("Message-ID".into()),
                header_value,
                true,
            );
        }

        Ok(())
    }

    pub(crate) fn exec_extracttext(
        &mut self,
        script: &'x Sieve<'x>,
        extract: &ops::ExtractText,
    ) -> Result<(), RuntimeError> {
        let mut value = String::new();

        if !self.part_iter_stack.is_empty() {
            match self.message.parts.get(self.part as usize).map(|p| &p.body) {
                Some(PartType::Text(text)) => {
                    value = if let Some(first) = extract.first {
                        text.chars().take(first as usize).collect()
                    } else {
                        text.as_ref().to_string()
                    };
                }
                Some(PartType::Html(html)) => {
                    value = if let Some(first) = extract.first {
                        html_to_text(html.as_ref())
                            .chars()
                            .take(first as usize)
                            .collect()
                    } else {
                        html_to_text(html.as_ref())
                    };
                }
                _ => (),
            }

            if !extract.modifiers.is_empty() && !value.is_empty() {
                let modified = self.apply_modifiers(script, extract.modifiers, value.into())?;
                return self.assign_variable(script, extract.name, modified);
            }
        }

        self.assign_variable(script, extract.name, value.into())
    }

    fn generate_message_id(&self) -> String {
        #[cfg(not(test))]
        {
            let mut header_value = Vec::with_capacity(self.runtime.local_hostname.len() + 64);
            platform::write_message_id(&mut header_value, &self.runtime.local_hostname);
            String::from_utf8(header_value).unwrap_or_default()
        }
        #[cfg(test)]
        {
            "<auto-generated@message-id>".to_string()
        }
    }

    pub(crate) fn build_message_id(&mut self) -> Option<Action<'x>> {
        if self.has_changes {
            self.last_message_id += 1;
            self.main_message_id = self.last_message_id;
            self.has_changes = false;
            let message = self.build_message();
            Some(Action::CreatedMessage {
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
                    message.extend_from_slice(b"\r\n");
                }

                if part.offset_body != 0 {
                    if let PartType::Multipart(subparts) = &part.body {
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
                            iter_stack.push((
                                StackItem::Message(current_message),
                                part,
                                std::mem::replace(&mut iter, [0].iter()),
                            ));
                            current_message = nested_message;
                            continue 'outer;
                        }
                        PartType::Multipart(subparts) => {
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

                            iter_stack.push((
                                StackItem::Boundary(prev_boundary),
                                part,
                                std::mem::replace(&mut iter, subparts.iter()),
                            ));
                            continue 'outer;
                        }
                        _ => {
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

        if last_offset > 0
            && let Some(bytes) = current_message.raw_message.get(last_offset as usize..)
        {
            message.extend_from_slice(bytes);
        }

        message
    }
}

enum StackItem<'x> {
    Message(&'x Message<'x>),
    Boundary(&'x str),
    None,
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
