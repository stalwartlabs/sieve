/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::{DateTime, HeaderName};

use crate::{
    compiler::grammar::actions::action_redirect::{ByTime, Redirect},
    Context, Event, Recipient,
};

impl Redirect {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        if let Some(address) = sanitize_address(ctx.eval_value(&self.address).to_string().as_ref())
        {
            if ctx.num_redirects < ctx.runtime.max_redirects
                && ctx.num_out_messages < ctx.runtime.max_out_messages
                && ctx.message.parts[0]
                    .headers
                    .iter()
                    .filter(|h| matches!(&h.name, HeaderName::Received))
                    .count()
                    < ctx.runtime.max_received_headers
            {
                // Try to avoid forwarding loops
                if !self.list && address.eq_ignore_ascii_case(ctx.user_address.as_ref()) {
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
                                ctx.eval_value(alimit).to_string().as_ref(),
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
