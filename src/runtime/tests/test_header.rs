use std::borrow::Cow;

use mail_parser::{
    parsers::{fields::unstructured::parse_unstructured, message::MessageStream},
    Header, HeaderName, HeaderValue, Message, RfcHeader,
};

use crate::{
    compiler::{
        grammar::{actions::action_mime::MimeOpts, tests::test_header::TestHeader, MatchType},
        lexer::string::StringItem,
    },
    Context,
};

use super::mime::SubpartIterator;

impl TestHeader {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) -> bool {
        let key_list = ctx.eval_strings(&self.key_list);
        let header_list = ctx.parse_header_names(&self.header_list);
        let mime_opts = match &self.mime_opts {
            MimeOpts::Type => MimeOpts::Type,
            MimeOpts::Subtype => MimeOpts::Subtype,
            MimeOpts::ContentType => MimeOpts::ContentType,
            MimeOpts::Param(params) => MimeOpts::Param(ctx.eval_strings(params)),
            MimeOpts::None => MimeOpts::None,
        };

        (match &self.match_type {
            MatchType::Is | MatchType::Contains => {
                let is_is = matches!(&self.match_type, MatchType::Is);
                message.find_headers(
                    ctx,
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header| {
                        message.find_header_values(header, &mime_opts, |value| {
                            for key in &key_list {
                                if is_is {
                                    if self.comparator.is(value, key.as_ref()) {
                                        return true;
                                    }
                                } else if self.comparator.contains(value, key.as_ref()) {
                                    return true;
                                }
                            }
                            false
                        })
                    },
                )
            }
            MatchType::Value(rel_match) => message.find_headers(
                ctx,
                &header_list,
                self.index,
                self.mime_anychild,
                |header| {
                    message.find_header_values(header, &mime_opts, |value| {
                        for key in &key_list {
                            if self.comparator.relational(rel_match, value, key.as_ref()) {
                                return true;
                            }
                        }
                        false
                    })
                },
            ),
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_positions = Vec::new();
                let is_matches = matches!(&self.match_type, MatchType::Matches(_));
                let result = message.find_headers(
                    ctx,
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header| {
                        message.find_header_values(header, &mime_opts, |value| {
                            for key in &key_list {
                                if is_matches {
                                    if self.comparator.matches(
                                        value,
                                        key.as_ref(),
                                        *capture_positions,
                                        &mut captured_positions,
                                    ) {
                                        return true;
                                    }
                                } else if self.comparator.regex(
                                    value,
                                    key.as_ref(),
                                    *capture_positions,
                                    &mut captured_positions,
                                ) {
                                    return true;
                                }
                            }
                            false
                        })
                    },
                );
                if !captured_positions.is_empty() {
                    ctx.set_match_variables(captured_positions);
                }
                result
            }
            MatchType::Count(rel_match) => {
                let mut count = 0;
                message.find_headers(
                    ctx,
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header| {
                        match &mime_opts {
                            MimeOpts::None => {
                                count += 1;
                            }
                            MimeOpts::Type | MimeOpts::Subtype | MimeOpts::ContentType => {
                                if let HeaderValue::ContentType(_) = &header.value {
                                    count += 1;
                                }
                            }
                            MimeOpts::Param(params) => {
                                if let HeaderValue::ContentType(ct) = &header.value {
                                    if let Some(attributes) = &ct.attributes {
                                        for (attr_name, _) in attributes {
                                            if params
                                                .iter()
                                                .any(|p| p.eq_ignore_ascii_case(attr_name))
                                            {
                                                count += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        false
                    },
                );

                let mut result = false;
                for key in &key_list {
                    if rel_match.cmp_num(count as f64, key.as_ref()) {
                        result = true;
                        break;
                    }
                }
                result
            }
            MatchType::List => false, //TODO: implement
        }) ^ self.is_not
    }
}

impl<'x> Context<'x> {
    pub(crate) fn parse_header_names<'z: 'y, 'y>(
        &'z self,
        header_names: &'y [StringItem],
    ) -> Vec<HeaderName<'y>> {
        let mut result = Vec::with_capacity(header_names.len());
        for header_name in header_names {
            let header_name = self.eval_string(header_name);
            result.push(
                if let Some(rfc_header) = RfcHeader::parse(header_name.as_ref()) {
                    HeaderName::Rfc(rfc_header)
                } else {
                    HeaderName::Other(header_name)
                },
            );
        }
        result
    }
}

pub(crate) trait MessageHeaders {
    fn find_headers(
        &self,
        ctx: &Context,
        header_names: &[HeaderName],
        index: Option<i32>,
        any_child: bool,
        visitor_fnc: impl FnMut(&Header) -> bool,
    ) -> bool;

    fn find_header_values(
        &self,
        header: &Header,
        mime_opts: &MimeOpts<Cow<str>>,
        visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool;
}

impl<'x> MessageHeaders for Message<'x> {
    fn find_headers(
        &self,
        ctx: &Context,
        header_names: &[HeaderName],
        index: Option<i32>,
        any_child: bool,
        mut visitor_fnc: impl FnMut(&Header) -> bool,
    ) -> bool {
        let parts = [ctx.part];
        let mut part_iter = SubpartIterator::new(ctx, self, &parts, any_child);

        while let Some((part_id, message_part)) = part_iter.next() {
            'outer: for header_name in header_names {
                match index {
                    None => {
                        for header in ctx.get_inserted_headers_top(part_id) {
                            if &header.name == header_name && visitor_fnc(header) {
                                return true;
                            }
                        }

                        for header in message_part
                            .headers
                            .iter()
                            .filter(|h| &h.name == header_name)
                        {
                            if !ctx.is_header_deleted(header.offset_field) && visitor_fnc(header) {
                                return true;
                            }
                        }

                        for header in ctx.get_inserted_headers_bottom(part_id) {
                            if &header.name == header_name && visitor_fnc(header) {
                                return true;
                            }
                        }
                    }
                    Some(index) if index >= 0 => {
                        let mut header_count = 0;

                        for header in ctx.get_inserted_headers_top(part_id) {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    continue 'outer;
                                }
                            }
                        }

                        for header in &message_part.headers {
                            if &header.name == header_name
                                && !ctx.is_header_deleted(header.offset_field)
                            {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    continue 'outer;
                                }
                            }
                        }

                        for header in ctx.get_inserted_headers_bottom(part_id) {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    continue 'outer;
                                }
                            }
                        }
                    }
                    Some(index) => {
                        let index = -index;
                        let mut header_count = 0;

                        for header in ctx.get_inserted_headers_bottom_rev(part_id) {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    continue 'outer;
                                }
                            }
                        }

                        for header in message_part.headers.iter().rev() {
                            if &header.name == header_name
                                && !ctx.is_header_deleted(header.offset_field)
                            {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    break;
                                }
                            }
                        }

                        for header in ctx.get_inserted_headers_top_rev(part_id) {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header) {
                                        return true;
                                    }
                                    continue 'outer;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    fn find_header_values(
        &self,
        header: &Header,
        mime_opts: &MimeOpts<Cow<str>>,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        match (mime_opts, &header.value) {
            (MimeOpts::None, HeaderValue::Text(text))
                if matches!(
                    &header.name,
                    HeaderName::Rfc(
                        RfcHeader::Subject
                            | RfcHeader::Comments
                            | RfcHeader::ContentDescription
                            | RfcHeader::ContentLocation
                            | RfcHeader::ContentTransferEncoding,
                    )
                ) || header.offset_end == 0 =>
            {
                visitor_fnc(text.as_ref())
            }
            (MimeOpts::None, _) => {
                if let HeaderValue::Text(text) = parse_unstructured(&mut MessageStream::new(
                    self.raw_message
                        .get(header.offset_start..header.offset_end)
                        .unwrap_or(b""),
                )) {
                    visitor_fnc(text.as_ref())
                } else {
                    visitor_fnc("")
                }
            }
            (MimeOpts::Type, HeaderValue::ContentType(ct)) => visitor_fnc(ct.c_type.as_ref()),
            (MimeOpts::Subtype, HeaderValue::ContentType(ct)) => {
                visitor_fnc(ct.c_subtype.as_deref().unwrap_or(""))
            }
            (MimeOpts::ContentType, HeaderValue::ContentType(ct)) => {
                if let Some(sub_type) = &ct.c_subtype {
                    visitor_fnc(&format!("{}/{}", ct.c_type, sub_type))
                } else {
                    visitor_fnc(ct.c_type.as_ref())
                }
            }
            (MimeOpts::Param(params), HeaderValue::ContentType(ct)) => {
                if let Some(attributes) = &ct.attributes {
                    for param in params {
                        for (attr_name, attr_value) in attributes {
                            if param.eq_ignore_ascii_case(attr_name)
                                && visitor_fnc(attr_value.as_ref())
                            {
                                return true;
                            }
                        }
                    }
                }
                visitor_fnc("")
            }
            _ => visitor_fnc(""),
        }
    }
}
