/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::borrow::Cow;

use mail_parser::{parsers::MessageStream, HeaderValue};

use crate::{
    compiler::grammar::tests::test_duplicate::{DupMatch, TestDuplicate},
    Context, Event,
};

use super::TestResult;

impl TestDuplicate {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let id: Cow<str> = match &self.dup_match {
            DupMatch::Header(header_name) => {
                let mut value = String::new();
                if let Some(header_name) = ctx.parse_header_name(header_name) {
                    ctx.find_headers(&[header_name], None, true, |header, _, _| {
                        if header.offset_end > 0 {
                            if let Some(bytes) = ctx
                                .message
                                .raw_message
                                .get(header.offset_start..header.offset_end)
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
