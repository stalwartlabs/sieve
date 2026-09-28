/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    Context, Sieve,
    bytecode::ops,
    compiler::grammar::Comparator,
    runtime::{RuntimeError, tests::test_header::MimeOptsRef},
};
use mail_parser::{HeaderName, PartId};

impl<'x> Context<'x> {
    pub(crate) fn exec_addheader(
        &mut self,
        script: &'x Sieve<'x>,
        add: &ops::AddHeader,
    ) -> Result<(), RuntimeError> {
        let header_name_ = self.eval_value(script, add.field_name)?;
        let header_name_ = header_name_.to_string();
        let mut header_name = String::with_capacity(header_name_.len());

        for ch in header_name_.chars() {
            if ch.is_alphanumeric() || ch == '-' {
                header_name.push(ch);
            }
        }

        if !header_name.is_empty()
            && let Some(header_name) = HeaderName::parse(self.alloc_string(header_name))
            && !self.runtime.protected_headers.contains(&header_name)
        {
            let header_value = self.alloc_string(
                self.eval_value(script, add.value)?
                    .to_string()
                    .as_ref()
                    .remove_crlf(self.runtime.max_header_size),
            );
            self.has_changes = true;
            self.insert_header(self.part, header_name, header_value, add.last);
        }

        Ok(())
    }

    pub(crate) fn exec_deleteheader(
        &mut self,
        script: &'x Sieve<'x>,
        delete: &ops::DeleteHeader,
    ) -> Result<(), RuntimeError> {
        let header_name_ = self.eval_value(script, delete.field_name)?;
        let header_name_ = header_name_.to_string();
        let Some(header_name) = HeaderName::parse(header_name_.as_ref()) else {
            return Ok(());
        };
        let value_patterns = self.eval_keys(script, delete.value_patterns)?;
        let comparator = Comparator::from_code(delete.comparator);
        let match_type = delete.match_type.match_type();
        let mut deleted_headers = Vec::new();
        let mut deleted_bytes = 0;
        let mut error = None;

        if self.runtime.protected_headers.contains(&header_name) {
            return Ok(());
        }

        self.find_headers(
            &[header_name],
            delete.index,
            delete.mime_anychild,
            |header, part_id| {
                if !value_patterns.is_empty() {
                    let did_match = self.find_header_values(header, &MimeOptsRef::None, |value| {
                        for key in &value_patterns {
                            match self.key_matches(
                                script,
                                &comparator,
                                &match_type,
                                key,
                                value,
                                &mut Vec::new(),
                            ) {
                                Ok(true) => return true,
                                Ok(false) => (),
                                Err(err) => {
                                    error = Some(err);
                                    return true;
                                }
                            }
                        }
                        false
                    });

                    if error.is_some() {
                        return true;
                    }
                    if !did_match {
                        return false;
                    }
                }

                deleted_bytes += header.len();
                deleted_headers.push((part_id, header));

                false
            },
        );

        if let Some(err) = error {
            return Err(err);
        }

        if !deleted_headers.is_empty() {
            self.has_changes = true;
            for (part_id, deleted) in deleted_headers {
                if let Some(part) = self.message.part(part_id) {
                    self.edits
                        .headers_mut(part)
                        .retain(|header| !header.same(&deleted));
                }
            }
        }

        self.message_size = self.message_size.saturating_sub(deleted_bytes);
        Ok(())
    }

    pub(crate) fn insert_header(
        &mut self,
        part_id: PartId,
        header_name: HeaderName<'x>,
        header_value: &'x str,
        last: bool,
    ) {
        let Some(part) = self.message.part(part_id) else {
            return;
        };
        let header = self.add_header(header_name, header_value);
        self.message_size += header.len();
        let headers = self.edits.headers_mut(part);
        if last {
            headers.push(header);
        } else {
            headers.insert(0, header);
        }
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
