/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{
    headers::HeaderRef,
    parts::{ROOT_PART, subtree_end},
};
use crate::Context;
use mail_parser::MessagePart;

impl<'x> Context<'x> {
    pub(crate) fn build_message(&self) -> Vec<u8> {
        let raw = self.message.raw();
        let mut message = Vec::with_capacity(self.message_size);
        match self.root_part() {
            Some(root) => {
                message.extend_from_slice(slice(raw, 0, root.offset_header()));
                self.write_part(&mut message, root);
                message
                    .extend_from_slice(raw.get(root.offset_end() as usize..).unwrap_or_default());
            }
            None => message.extend_from_slice(raw),
        }
        message
    }

    fn write_part(&self, out: &mut Vec<u8>, part: MessagePart<'x>) {
        let id = part.id();
        if !self.edits.touches(&(id..subtree_end(part))) {
            out.extend_from_slice(part.raw());
            return;
        }

        let source = part.message().source_bytes();
        let body = self.edits.body(id);
        match self.edits.headers(id) {
            Some(headers) => {
                let (mut count_left, mut size_left) =
                    if id == ROOT_PART && self.runtime.has_header_block_limits() {
                        headers
                            .iter()
                            .filter(|header| !matches!(header, HeaderRef::Added(_)))
                            .fold(
                                (
                                    self.runtime.max_header_count,
                                    self.runtime.max_header_block_size,
                                ),
                                |(count, size), header| {
                                    (count.saturating_sub(1), size.saturating_sub(header.len()))
                                },
                            )
                    } else {
                        (usize::MAX, usize::MAX)
                    };
                for header in headers {
                    if matches!(header, HeaderRef::Added(_)) {
                        let len = header.len();
                        if count_left == 0 || size_left < len {
                            count_left = 0;
                            continue;
                        }
                        count_left -= 1;
                        size_left -= len;
                    }
                    header.write(source, out);
                }
                if !body.is_some_and(|body| body.mime) {
                    out.extend_from_slice(b"\r\n");
                }
            }
            None => out.extend_from_slice(part.raw_headers()),
        }

        if let Some(body) = body {
            out.extend_from_slice(body.text.as_bytes());
        } else if part.is_multipart() {
            let mut offset = part.offset_body();
            for child in part.children() {
                out.extend_from_slice(slice(source, offset, child.offset_header()));
                self.write_part(out, child);
                offset = child.offset_end();
            }
            out.extend_from_slice(slice(source, offset, part.offset_end()));
        } else {
            out.extend_from_slice(part.raw_body());
        }
    }
}

#[inline(always)]
fn slice(bytes: &[u8], start: u32, end: u32) -> &[u8] {
    bytes.get(start as usize..end as usize).unwrap_or_default()
}
