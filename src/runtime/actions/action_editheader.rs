/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::borrow::Cow;

use mail_parser::{Header, HeaderName, HeaderValue};

use crate::{
    compiler::grammar::{
        actions::{
            action_editheader::{AddHeader, DeleteHeader},
            action_mime::MimeOpts,
        },
        MatchType,
    },
    Context,
};

impl AddHeader {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let header_name__ = ctx.eval_value(&self.field_name);
        let header_name_ = header_name__.to_string();
        let mut header_name = String::with_capacity(header_name_.len());

        for ch in header_name_.chars() {
            if ch.is_alphanumeric() || ch == '-' {
                header_name.push(ch);
            }
        }

        if !header_name.is_empty() {
            if let Some(header_name) = HeaderName::parse(header_name) {
                if !ctx.runtime.protected_headers.contains(&header_name) {
                    ctx.has_changes = true;
                    ctx.insert_header(
                        ctx.part,
                        header_name,
                        ctx.eval_value(&self.value)
                            .to_string()
                            .as_ref()
                            .remove_crlf(ctx.runtime.max_header_size),
                        self.last,
                    )
                }
            }
        }
    }
}

impl DeleteHeader {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let header_name__ = ctx.eval_value(&self.field_name);
        let header_name_ = header_name__.to_string();
        let header_name = if let Some(header_name) = HeaderName::parse(header_name_.as_ref()) {
            header_name
        } else {
            return;
        };
        let value_patterns = ctx.eval_values(&self.value_patterns);
        let mut deleted_headers = Vec::new();
        let mut deleted_bytes = 0;

        if ctx.runtime.protected_headers.contains(&header_name) {
            return;
        }

        ctx.find_headers(
            &[header_name],
            self.index,
            self.mime_anychild,
            |header, part_id, header_pos| {
                if !value_patterns.is_empty() {
                    let did_match = ctx.find_header_values(header, &MimeOpts::None, |value| {
                        for (pattern_expr, pattern) in
                            value_patterns.iter().zip(self.value_patterns.iter())
                        {
                            if match &self.match_type {
                                MatchType::Is => self.comparator.is(&value, pattern_expr),
                                MatchType::Contains => self
                                    .comparator
                                    .contains(value, pattern_expr.to_string().as_ref()),
                                MatchType::Value(rel_match) => {
                                    self.comparator.relational(rel_match, &value, pattern_expr)
                                }
                                MatchType::Matches(_) => self.comparator.matches(
                                    value,
                                    pattern_expr.to_string().as_ref(),
                                    0,
                                    &mut Vec::new(),
                                ),
                                MatchType::Regex(_) => self.comparator.regex(
                                    pattern,
                                    pattern_expr,
                                    value,
                                    0,
                                    &mut Vec::new(),
                                ),
                                MatchType::Count(_) => false,
                                MatchType::List => false,
                            } {
                                return true;
                            }
                        }
                        false
                    });

                    if !did_match {
                        return false;
                    }
                }

                if header.offset_end != 0 {
                    deleted_bytes += (header.offset_end - header.offset_field) as usize;
                } else {
                    deleted_bytes += header.name.as_str().len() + header.value.len() + 4;
                }
                deleted_headers.push((part_id, header_pos));

                false
            },
        );

        if !deleted_headers.is_empty() {
            ctx.has_changes = true;
            for (part_id, header_pos) in deleted_headers.iter().rev() {
                ctx.message.parts[*part_id as usize]
                    .headers
                    .remove(*header_pos);
            }
        }

        ctx.message_size -= deleted_bytes;
    }
}

pub(crate) trait RemoveCrLf {
    fn remove_crlf(&self, max_len: usize) -> String;
}

impl RemoveCrLf for &str {
    fn remove_crlf(&self, max_len: usize) -> String {
        let mut header_value = String::with_capacity(self.len());
        for ch in self.chars() {
            if !['\n', '\r'].contains(&ch) {
                if header_value.len() + ch.len_utf8() <= max_len {
                    header_value.push(ch);
                } else {
                    return header_value;
                }
            }
        }
        header_value
    }
}

impl<'x> Context<'x> {
    pub(crate) fn insert_header(
        &mut self,
        part_id: u32,
        header_name: HeaderName<'x>,
        header_value: impl Into<Cow<'static, str>>,
        last: bool,
    ) {
        let header_value = header_value.into();
        self.message_size += header_name.len() + header_value.len() + 4;
        let header = Header {
            name: header_name,
            value: HeaderValue::Text(header_value),
            offset_start: 0,
            offset_end: 0,
            offset_field: 0,
        };

        if !last {
            self.message.parts[part_id as usize]
                .headers
                .insert(0, header);
        } else {
            self.message.parts[part_id as usize].headers.push(header);
        }
    }
}
