use mail_parser::{Header, HeaderName, HeaderValue, Message};

use crate::{
    compiler::grammar::{
        actions::{
            action_editheader::{AddHeader, DeleteHeader},
            action_mime::MimeOpts,
        },
        MatchType,
    },
    runtime::tests::test_header::MessageHeaders,
    Context,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct InsertHeader<'x> {
    pub(crate) header: Header<'x>,
    pub(crate) last: bool,
    pub(crate) part_id: usize,
}

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
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) {
        let header_name = HeaderName::parse(ctx.eval_string(&self.field_name));
        let value_patterns = ctx.eval_strings(&self.value_patterns);
        let mut deletion_offsets = Vec::new();
        let mut deletion_ids = Vec::new();
        let mut deleted_bytes = 0;

        if ctx.runtime.protected_headers.contains(&header_name) {
            return;
        }

        message.find_headers(
            ctx,
            &[header_name],
            self.index,
            self.mime_anychild,
            |header| {
                if !value_patterns.is_empty() {
                    let did_match = message.find_header_values(header, &MimeOpts::None, |value| {
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
                    deletion_offsets.push(header.offset_field);
                    deleted_bytes += header.offset_end - header.offset_field;
                } else {
                    deletion_ids.push(header.offset_field);
                    deleted_bytes += header.name.as_str().len() + header.value.len() + 4;
                }

                false
            },
        );

        if !deletion_offsets.is_empty() {
            ctx.header_deletions.extend(deletion_offsets);
        }

        if !deletion_ids.is_empty() {
            let mut new_header_insertions = Vec::new();
            let mut new_header_id = 0;
            for mut header in ctx.header_insertions.drain(..) {
                if !deletion_ids.contains(&header.header.offset_field) {
                    header.header.offset_field = new_header_id;
                    new_header_insertions.push(header);
                    new_header_id += 1;
                }
            }
            ctx.header_insertions = new_header_insertions;
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
        self.header_insertions.push(InsertHeader {
            header: Header {
                name: header_name,
                value: HeaderValue::Text(header_value.into()),
                offset_start: 0, // 0 = Insert header marker
                offset_end: 0,   // 0 = Insert header marker
                offset_field: self.header_insertions.len(),
            },
            last,
            part_id,
        });
    }

    pub(crate) fn get_inserted_headers(&self, part_id: usize) -> impl Iterator<Item = &Header> {
        self.header_insertions.iter().filter_map(move |hi| {
            if hi.part_id == part_id {
                Some(&hi.header)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_inserted_headers_top(&self, part_id: usize) -> impl Iterator<Item = &Header> {
        self.header_insertions.iter().filter_map(move |hi| {
            if hi.part_id == part_id && !hi.last {
                Some(&hi.header)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_inserted_headers_bottom(
        &self,
        part_id: usize,
    ) -> impl Iterator<Item = &Header> {
        self.header_insertions.iter().rev().filter_map(move |hi| {
            if hi.part_id == part_id && hi.last {
                Some(&hi.header)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_inserted_headers_top_rev(
        &self,
        part_id: usize,
    ) -> impl Iterator<Item = &Header> {
        self.header_insertions.iter().rev().filter_map(move |hi| {
            if hi.part_id == part_id && !hi.last {
                Some(&hi.header)
            } else {
                None
            }
        })
    }

    pub(crate) fn get_inserted_headers_bottom_rev(
        &self,
        part_id: usize,
    ) -> impl Iterator<Item = &Header> {
        self.header_insertions.iter().filter_map(move |hi| {
            if hi.part_id == part_id && hi.last {
                Some(&hi.header)
            } else {
                None
            }
        })
    }

    pub(crate) fn is_header_deleted(&self, offset: usize) -> bool {
        self.header_deletions.contains(&offset)
    }
}
