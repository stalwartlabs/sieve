use mail_parser::{
    decoders::html::html_to_text, Encoding, Header, HeaderName, HeaderValue, Message, MessagePart,
    PartType, RfcHeader,
};

use crate::{
    compiler::grammar::actions::{
        action_mime::{Enclose, ExtractText, Replace},
        action_set::Variable,
    },
    Context,
};

use super::action_editheader::RemoveCrLf;

impl Replace {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        // Delete children parts
        for part_id in ctx.find_nested_parts_ids(false).iter().rev() {
            ctx.message.parts.remove(*part_id);
        }

        // Update part
        let body = ctx.eval_string(&self.replacement).into_owned();
        let body_len = body.len();
        ctx.message_size += body_len;

        let part = &mut ctx.message.parts[ctx.part];

        ctx.message_size -= if part.offset_body != 0 {
            part.offset_end - part.offset_header
        } else {
            part.body.len()
        };
        part.body = PartType::Text(body.into());
        part.encoding = if !self.mime {
            Encoding::QuotedPrintable
        } else {
            Encoding::None
        };
        part.offset_body = 0;
        let prev_headers = std::mem::take(&mut part.headers);

        if ctx.part == 0 {
            for mut header in prev_headers {
                let mut size = 0;
                match &header.name {
                    HeaderName::Rfc(RfcHeader::Subject) => {
                        if self.subject.is_some() {
                            header.name = HeaderName::Other("Original-Subject".into());
                            size += "Original-".len();
                        }
                    }
                    HeaderName::Rfc(RfcHeader::From) => {
                        if self.from.is_some() {
                            header.name = HeaderName::Other("Original-From".into());
                            size += "Original-".len();
                        }
                    }
                    HeaderName::Rfc(
                        RfcHeader::To | RfcHeader::Cc | RfcHeader::Bcc | RfcHeader::Received,
                    ) => (),
                    _ => continue,
                }
                ctx.message_size += (header.offset_end - header.offset_field) + size;
                part.headers.push(header);
            }

            if let Some(from) = self.from.as_ref().map(|f| ctx.eval_string(f)) {
                if !from.is_empty() {
                    ctx.insert_header(
                        0,
                        HeaderName::Other("From".into()),
                        from.as_ref().remove_crlf(),
                        false,
                    );
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
                }
            }
        }

        if !self.mime {
            ctx.insert_header(
                ctx.part,
                HeaderName::Other("Content-Type".into()),
                "text/plain; charset=utf-8".to_string(),
                false,
            );
        }
    }
}

impl Enclose {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let body = ctx.eval_string(&self.value).into_owned();
        let subject = self
            .subject
            .as_ref()
            .map(|s| ctx.eval_string(s).into_owned())
            .or_else(|| ctx.message.get_subject().map(|s| s.to_string()))
            .unwrap_or_default();

        let message = std::mem::take(&mut ctx.message);
        ctx.part = 0;
        ctx.message = Message {
            html_body: vec![1],
            text_body: vec![1],
            attachments: vec![2],
            parts: vec![
                MessagePart {
                    headers: vec![Header {
                        name: HeaderName::Other("Content-Type".into()),
                        value: HeaderValue::Text("multipart/mixed; boundary=\"\"".into()),
                        offset_field: 0,
                        offset_start: 0,
                        offset_end: 0,
                    }],
                    is_encoding_problem: false,
                    body: PartType::Multipart(vec![1, 2]),
                    encoding: Encoding::None,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
                MessagePart {
                    headers: vec![Header {
                        name: HeaderName::Other("Content-Type".into()),
                        value: HeaderValue::Text("text/plain; charset=utf-8".into()),
                        offset_field: 0,
                        offset_start: 0,
                        offset_end: 0,
                    }],
                    is_encoding_problem: false,
                    body: PartType::Text(body.into()),
                    encoding: Encoding::None,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
                MessagePart {
                    headers: vec![Header {
                        name: HeaderName::Other("Content-Type".into()),
                        value: HeaderValue::Text("message/rfc822".into()),
                        offset_field: 0,
                        offset_start: 0,
                        offset_end: 0,
                    }],
                    is_encoding_problem: false,
                    body: PartType::Message(message),
                    encoding: Encoding::None,
                    offset_header: 0,
                    offset_body: 0,
                    offset_end: 0,
                },
            ],
            raw_message: b""[..].into(),
        };

        // TODO: update message size
        ctx.insert_header(0, HeaderName::Other("Subject".into()), subject, false);

        for header in &self.headers {
            let header = ctx.eval_string(header);
            if let Some((mut header_name, mut header_value)) = header.as_ref().split_once(':') {
                header_name = header_name.trim();
                header_value = header_value.trim();
                if !header_value.is_empty() {
                    let header_name = HeaderName::parse(header_name.to_string());
                    if !ctx.runtime.protected_headers.contains(&header_name) {
                        ctx.insert_header(0, header_name, header_value.to_string(), false);
                    }
                }
            }
        }
    }
}

impl ExtractText {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let mut value = String::new();

        if !ctx.part_iter_stack.is_empty() {
            match ctx.message.parts.get(ctx.part).map(|p| &p.body) {
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
