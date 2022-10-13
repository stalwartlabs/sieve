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
        let header_name_ = ctx.eval_string(&self.field_name);
        let mut header_name = String::with_capacity(header_name_.len());

        for ch in header_name_.chars() {
            if ch.is_alphanumeric() || ch == '-' {
                header_name.push(ch);
            }
        }

        if !header_name.is_empty() {
            let header_name = HeaderName::parse(header_name);

            if !ctx.runtime.protected_headers.contains(&header_name) {
                ctx.insert_header(
                    ctx.part,
                    header_name,
                    ctx.eval_string(&self.value).as_ref().remove_crlf(),
                    self.last,
                )
            }
        }
    }
}

impl DeleteHeader {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        let header_name = HeaderName::parse(ctx.eval_string(&self.field_name));
        let value_patterns = ctx.eval_strings(&self.value_patterns);
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
                        for pattern in &value_patterns {
                            if match &self.match_type {
                                MatchType::Is => self.comparator.is(value, pattern.as_ref()),
                                MatchType::Contains => {
                                    self.comparator.contains(value, pattern.as_ref())
                                }
                                MatchType::Value(rel_match) => {
                                    self.comparator
                                        .relational(rel_match, value, pattern.as_ref())
                                }
                                MatchType::Matches(_) => self.comparator.matches(
                                    value,
                                    pattern.as_ref(),
                                    0,
                                    &mut Vec::new(),
                                ),
                                MatchType::Regex(_) => self.comparator.regex(
                                    value,
                                    pattern.as_ref(),
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
                    deleted_bytes += header.offset_end - header.offset_field;
                } else {
                    deleted_bytes += header.name.as_str().len() + header.value.len() + 4;
                }
                deleted_headers.push((part_id, header_pos));

                false
            },
        );

        for (part_id, header_pos) in deleted_headers.iter().rev() {
            ctx.message.parts[*part_id].headers.remove(*header_pos);
        }

        ctx.message_size -= deleted_bytes;
    }
}

pub(crate) trait RemoveCrLf {
    fn remove_crlf(&self) -> String;
}

impl RemoveCrLf for &str {
    fn remove_crlf(&self) -> String {
        let mut header_value = String::with_capacity(self.len());
        for ch in self.chars() {
            if !['\n', '\r'].contains(&ch) {
                header_value.push(ch);
            }
        }
        header_value
    }
}

impl<'x> Context<'x> {
    pub(crate) fn insert_header(
        &mut self,
        part_id: usize,
        header_name: HeaderName<'x>,
        header_value: String,
        last: bool,
    ) {
        self.message_size += header_name.len() + header_value.len() + 4;
        let header = Header {
            name: header_name,
            value: HeaderValue::Text(header_value.into()),
            offset_start: 0,
            offset_end: 0,
            offset_field: 0,
        };

        if !last {
            self.message.parts[part_id].headers.insert(0, header);
        } else {
            self.message.parts[part_id].headers.push(header);
        }
    }
}
