use mail_parser::{parsers::MessageStream, HeaderValue};

use crate::{
    compiler::grammar::tests::test_duplicate::{DupMatch, TestDuplicate},
    Context, Event, Expiry,
};

use super::TestResult;

impl TestDuplicate {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let id = match &self.dup_match {
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
                            let bytes = format!("{}\n", text).into_bytes();
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
            DupMatch::UniqueId(s) => ctx.eval_string(s),
            DupMatch::Default => ctx.message.get_message_id().unwrap_or("").into(),
        };

        TestResult::Event {
            event: Event::DuplicateId {
                id: if id.is_empty() {
                    return TestResult::Bool(false ^ self.is_not);
                } else if let Some(handle) = &self.handle {
                    format!("{}{}", ctx.eval_string(handle), id)
                } else {
                    id.into_owned()
                },
                expiry: match &self.seconds {
                    Some(seconds) if self.last => Expiry::Seconds(*seconds),
                    Some(seconds) => Expiry::LastSeconds(*seconds),
                    None => Expiry::None,
                },
            },
            is_not: self.is_not,
        }
    }
}
