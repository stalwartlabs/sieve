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

use mail_parser::{DateTime, HeaderName, RfcHeader};

use crate::{
    compiler::grammar::actions::action_redirect::{ByTime, Redirect},
    Context, Envelope, Event, Recipient,
};

impl Redirect {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        if let Some(address) = sanitize_address(ctx.eval_value(&self.address).into_cow().as_ref()) {
            if ctx.num_redirects < ctx.runtime.max_redirects
                && ctx.num_out_messages < ctx.runtime.max_out_messages
                && ctx.message.parts[0]
                    .headers
                    .iter()
                    .filter(|h| matches!(&h.name, HeaderName::Rfc(RfcHeader::Received)))
                    .count()
                    < ctx.runtime.max_received_headers
            {
                // Try to avoid fowarding loops
                if !self.list
                    && (address.eq_ignore_ascii_case(ctx.user_address.as_ref())
                        || ctx.envelope.iter().any(|(e, v)| {
                            matches!(e, Envelope::From)
                                && v.to_cow().eq_ignore_ascii_case(address.as_str())
                        }))
                {
                    return;
                }

                if !self.copy && matches!(&ctx.final_event, Some(Event::Keep { .. })) {
                    ctx.final_event = None;
                }

                let mut events = Vec::with_capacity(2);
                if let Some(event) = ctx.build_message_id() {
                    events.push(event);
                }
                ctx.num_redirects += 1;
                ctx.num_out_messages += 1;
                events.push(Event::SendMessage {
                    recipient: if !self.list {
                        Recipient::Address(address)
                    } else {
                        Recipient::List(address)
                    },
                    notify: self.notify.clone(),
                    return_of_content: self.return_of_content.clone(),
                    by_time: match &self.by_time {
                        ByTime::Relative {
                            rlimit,
                            mode,
                            trace,
                        } => ByTime::Relative {
                            rlimit: *rlimit,
                            mode: mode.clone(),
                            trace: *trace,
                        },
                        ByTime::Absolute {
                            alimit,
                            mode,
                            trace,
                        } => ByTime::Absolute {
                            alimit: DateTime::parse_rfc3339(
                                ctx.eval_value(alimit).into_cow().as_ref(),
                            )
                            .and_then(|d| {
                                if d.is_valid() {
                                    d.to_timestamp().into()
                                } else {
                                    None
                                }
                            })
                            .unwrap_or(0),
                            mode: mode.clone(),
                            trace: *trace,
                        },
                        ByTime::None => ByTime::None,
                    },
                    message_id: ctx.main_message_id,
                });
                ctx.queued_events = events.into_iter();
            }
        }
    }
}

pub(crate) fn sanitize_address(addr: &str) -> Option<String> {
    let mut result = String::with_capacity(addr.len());
    let mut in_quote = false;
    let mut last_ch = '\n';
    let mut has_at = false;
    let mut has_dot = false;

    for ch in addr.chars() {
        match ch {
            '\"' => {
                if !in_quote {
                    in_quote = true;
                } else if last_ch != '\\' {
                    in_quote = false;
                }
            }
            '@' if !in_quote => {
                if !has_at && !result.is_empty() {
                    has_at = true;
                    result.push(ch);
                } else {
                    return None;
                }
            }
            '.' if !in_quote && has_at && !has_dot => {
                has_dot = true;
                result.push(ch);
            }
            '<' => {
                result.clear();
                has_at = false;
                has_dot = false;
            }
            '>' => (),
            _ => {
                if !ch.is_ascii_whitespace() || in_quote {
                    result.push(ch);
                }
            }
        }
        last_ch = ch;
    }

    if !result.is_empty() && has_at && has_dot {
        Some(result)
    } else {
        None
    }
}
