/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::borrow::Cow;

use mail_parser::{parsers::MessageStream, HeaderValue};

use crate::{
    compiler::grammar::tests::test_duplicate::{DupMatch, TestDuplicate},
    Context, Event,
};

use super::TestResult;

impl TestDuplicate {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let id: Cow<str> = match &self.dup_match {
            DupMatch::Header(header_name) => {
                let mut value = String::new();
                if let Some(header_name) = ctx.parse_header_name(header_name) {
                    ctx.find_headers(&[header_name], None, true, |header, _, _| {
                        if header.offset_end > 0 {
                            if let Some(bytes) = ctx
                                .message
                                .raw_message
                                .get(header.offset_start as usize..header.offset_end as usize)
                            {
                                if let HeaderValue::Text(id) = MessageStream::new(bytes).parse_id()
                                {
                                    if !id.is_empty() {
                                        value = id.to_string();
                                        return true;
                                    }
                                }
                            }
                        } else if let HeaderValue::Text(text) = &header.value {
                            // Inserted header
                            let bytes = format!("{text}\n").into_bytes();
                            if let HeaderValue::Text(id) = MessageStream::new(&bytes).parse_id() {
                                if !id.is_empty() {
                                    value = id.to_string();
                                    return true;
                                }
                            }
                        }
                        false
                    });
                }
                value.into()
            }
            DupMatch::UniqueId(s) => ctx.eval_value(s).to_string().into_owned().into(),
            DupMatch::Default => ctx.message.message_id().unwrap_or("").into(),
        };

        TestResult::Event {
            event: Event::DuplicateId {
                id: if id.is_empty() {
                    return TestResult::Bool(false ^ self.is_not);
                } else if let Some(handle) = &self.handle {
                    format!("{}{}", ctx.eval_value(handle).to_string(), id)
                } else {
                    id.into_owned()
                },
                expiry: self.seconds.unwrap_or(ctx.runtime.default_duplicate_expiry),
                last: self.last,
            },
            is_not: self.is_not,
        }
    }
}
