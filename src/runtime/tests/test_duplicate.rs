/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{
    Context, Sieve,
    bytecode::{ops, rec::Rec},
    runtime::{RuntimeError, handler::Handler},
};
use mail_parser::{HeaderValue, parsers::MessageStream};
use std::borrow::Cow;

impl<'x> Context<'x> {
    pub(crate) fn test_duplicate<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestDuplicate,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let id = match test.dup_match.kind {
            0 => self.duplicate_header_id(script, test.dup_match.value)?,
            1 => self.eval_value(script, test.dup_match.value)?.into_string(),
            _ => self.message.message_id().unwrap_or("").into(),
        };

        if id.is_empty() {
            return Ok(TestResult::Bool(false ^ test.is_not));
        }

        let id = match self.eval_opt(script, test.handle)? {
            Some(handle) => {
                let mut prefixed = handle.to_string().into_owned();
                prefixed.push_str(&id);
                Cow::Owned(prefixed)
            }
            None => id,
        };
        let expiry = test
            .seconds
            .unwrap_or(self.runtime.default_duplicate_expiry);

        TestResult::from_reply(
            handler.duplicate_id(self, &id, expiry, test.last),
            test.is_not,
        )
    }

    fn duplicate_header_id(
        &self,
        script: &'x Sieve<'x>,
        header_name: Rec,
    ) -> Result<Cow<'_, str>, RuntimeError> {
        let mut value = Cow::Borrowed("");
        if let Some(header_name) = self.parse_header_name(script, header_name)? {
            self.find_headers(&[header_name], None, true, |header, _, _| {
                if header.offset_end > 0 {
                    if let Some(bytes) = self
                        .message
                        .raw_message
                        .get(header.offset_start as usize..header.offset_end as usize)
                        && let HeaderValue::Text(id) = MessageStream::new(bytes).parse_id()
                        && !id.is_empty()
                    {
                        value = id;
                        return true;
                    }
                } else if let HeaderValue::Text(text) = &header.value {
                    let bytes = format!("{text}\n").into_bytes();
                    if let HeaderValue::Text(id) = MessageStream::new(&bytes).parse_id()
                        && !id.is_empty()
                    {
                        value = Cow::Owned(id.into_owned());
                        return true;
                    }
                }
                false
            });
        }
        Ok(value)
    }
}
