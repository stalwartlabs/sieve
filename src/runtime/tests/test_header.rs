/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::{parsers::MessageStream, Header, HeaderName, HeaderValue};

use crate::{
    compiler::{
        grammar::{actions::action_mime::MimeOpts, tests::test_header::TestHeader, MatchType},
        Number, Value,
    },
    runtime::Variable,
    Context, Event,
};

use super::{mime::SubpartIterator, TestResult};

impl TestHeader {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let key_list = ctx.eval_values(&self.key_list);
        let header_list = ctx.parse_header_names(&self.header_list);
        let mime_opts = match &self.mime_opts {
            MimeOpts::Type => MimeOpts::Type,
            MimeOpts::Subtype => MimeOpts::Subtype,
            MimeOpts::ContentType => MimeOpts::ContentType,
            MimeOpts::Param(params) => MimeOpts::Param(ctx.eval_values(params)),
            MimeOpts::None => MimeOpts::None,
        };

        let result = match &self.match_type {
            MatchType::Is | MatchType::Contains => {
                let is_is = matches!(&self.match_type, MatchType::Is);
                ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_header_values(header, &mime_opts, |value| {
                            for key in &key_list {
                                if is_is {
                                    if self.comparator.is(&value, key) {
                                        return true;
                                    }
                                } else if self.comparator.contains(value, key.to_string().as_ref())
                                {
                                    return true;
                                }
                            }
                            false
                        })
                    },
                )
            }
            MatchType::Value(rel_match) => ctx.find_headers(
                &header_list,
                self.index,
                self.mime_anychild,
                |header, _, _| {
                    ctx.find_header_values(header, &mime_opts, |value| {
                        for key in &key_list {
                            if self.comparator.relational(rel_match, &value, key) {
                                return true;
                            }
                        }
                        false
                    })
                },
            ),
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_values = Vec::new();
                let is_matches = matches!(&self.match_type, MatchType::Matches(_));
                let result = ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_header_values(header, &mime_opts, |value| {
                            for (pattern_expr, pattern) in key_list.iter().zip(self.key_list.iter())
                            {
                                if is_matches {
                                    if self.comparator.matches(
                                        value,
                                        pattern_expr.to_string().as_ref(),
                                        *capture_positions,
                                        &mut captured_values,
                                    ) {
                                        return true;
                                    }
                                } else if self.comparator.regex(
                                    pattern,
                                    pattern_expr,
                                    value,
                                    *capture_positions,
                                    &mut captured_values,
                                ) {
                                    return true;
                                }
                            }
                            false
                        })
                    },
                );
                if !captured_values.is_empty() {
                    ctx.set_match_variables(captured_values);
                }
                result
            }
            MatchType::Count(rel_match) => {
                let mut count = 0;
                ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
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
                                        for attr in attributes {
                                            if params.iter().any(|p| {
                                                p.to_string().eq_ignore_ascii_case(&attr.name)
                                            }) {
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
                    if rel_match.cmp(&Number::from(count), &key.to_number()) {
                        result = true;
                        break;
                    }
                }
                result
            }
            MatchType::List => {
                let mut values: Vec<String> = Vec::new();
                ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_header_values(header, &mime_opts, |value| {
                            if !value.is_empty() && !values.iter().any(|v| v.eq(value)) {
                                values.push(value.to_string());
                            }
                            false
                        })
                    },
                );

                if !values.is_empty() {
                    return TestResult::Event {
                        event: Event::ListContains {
                            lists: ctx.eval_values_owned(&self.key_list),
                            values,
                            match_as: self.comparator.as_match(),
                        },
                        is_not: self.is_not,
                    };
                }

                false
            }
        };

        TestResult::Bool(result ^ self.is_not)
    }
}

impl Context<'_> {
    pub(crate) fn parse_header_names<'z: 'y, 'y>(
        &'z self,
        header_names: &'y [Value],
    ) -> Vec<HeaderName<'y>> {
        let mut result = Vec::with_capacity(header_names.len());
        for header_name in header_names {
            if let Some(header_name) = self.parse_header_name(header_name) {
                result.push(header_name);
            }
        }
        result
    }

    #[inline(always)]
    pub(crate) fn parse_header_name(&self, header_name: &Value) -> Option<HeaderName<'static>> {
        let h_ = self.eval_value(header_name);
        let h = h_.to_string();

        match HeaderName::parse(h.as_ref())? {
            HeaderName::Other(_) => HeaderName::Other(h.into_owned().into()),
            hn => hn.into_owned(),
        }
        .into()
    }

    pub(crate) fn find_headers(
        &self,
        header_names: &[HeaderName],
        index: Option<i32>,
        any_child: bool,
        mut visitor_fnc: impl FnMut(&Header, u32, usize) -> bool,
    ) -> bool {
        let parts = [self.part];
        let mut part_iter = SubpartIterator::new(self, &parts, any_child);

        while let Some((part_id, message_part)) = part_iter.next() {
            'outer: for header_name in header_names {
                match index {
                    None => {
                        for (pos, header) in message_part
                            .headers
                            .iter()
                            .enumerate()
                            .filter(|(_, h)| &h.name == header_name)
                        {
                            if visitor_fnc(header, part_id, pos) {
                                return true;
                            }
                        }
                    }
                    Some(index) if index >= 0 => {
                        let mut header_count = 0;

                        for (pos, header) in message_part.headers.iter().enumerate() {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header, part_id, pos) {
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

                        for (pos, header) in message_part.headers.iter().enumerate().rev() {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header, part_id, pos) {
                                        return true;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    #[allow(unused_assignments)]
    pub(crate) fn find_header_values(
        &self,
        header: &Header,
        mime_opts: &MimeOpts<Variable>,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        let mut raw_header = None;
        let mut header_value_ = None;
        let header_value = if header.offset_end != 0 {
            &header.value
        } else {
            let value = if let HeaderValue::Text(text) = &header.value {
                text.as_ref()
            } else {
                #[cfg(test)]
                panic!("Unexpected value.");
                #[cfg(not(test))]
                return false;
            };
            if mime_opts == &MimeOpts::None {
                return visitor_fnc(value);
            } else {
                raw_header = format!("{value}\n").into_bytes().into();
                header_value_ = MessageStream::new(raw_header.as_ref().unwrap())
                    .parse_content_type()
                    .into();
                header_value_.as_ref().unwrap()
            }
        };

        match (mime_opts, header_value) {
            (MimeOpts::None, HeaderValue::Text(text))
                if matches!(
                    &header.name,
                    HeaderName::Subject
                        | HeaderName::Comments
                        | HeaderName::ContentDescription
                        | HeaderName::ContentLocation
                        | HeaderName::ContentTransferEncoding,
                ) =>
            {
                visitor_fnc(text.as_ref())
            }
            (MimeOpts::None, _) => {
                if let HeaderValue::Text(text) = MessageStream::new(
                    self.message
                        .raw_message
                        .get(header.offset_start as usize..header.offset_end as usize)
                        .unwrap_or(b""),
                )
                .parse_unstructured()
                {
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
                        for attr in attributes {
                            if param.to_string().eq_ignore_ascii_case(&attr.name)
                                && visitor_fnc(attr.value.as_ref())
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
