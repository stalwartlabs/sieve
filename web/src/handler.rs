/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use std::collections::BTreeSet;

use sieve::{
    Context, Envelope, Handler, Importance, Mailbox, MatchAs, MessageSource, Recipient, Reply,
    SieveAction,
    compiler::grammar::actions::action_redirect::{ByMode, ByTime, Notify, NotifyItem, Ret},
};

use crate::{
    output::{Detail, Event, OutputMessage},
    settings::Settings,
};

pub struct Recorder<'s> {
    settings: &'s Settings,
    pub seen_ids: BTreeSet<String>,
    pub events: Vec<Event>,
    pub messages: Vec<OutputMessage>,
    pub after_error: bool,
}

impl<'s> Recorder<'s> {
    pub fn new(settings: &'s Settings, seen_ids: impl IntoIterator<Item = String>) -> Self {
        Recorder {
            settings,
            seen_ids: seen_ids.into_iter().collect(),
            events: Vec::new(),
            messages: Vec::new(),
            after_error: false,
        }
    }

    fn has_mailbox(&self, name: &str, special_use: &[&str]) -> bool {
        self.settings.mailboxes.iter().any(|mailbox| {
            (mailbox.name == name
                || (name.eq_ignore_ascii_case("INBOX")
                    && mailbox.name.eq_ignore_ascii_case("INBOX")))
                && special_use.iter().all(|wanted| {
                    mailbox
                        .special_use
                        .iter()
                        .any(|have| have.eq_ignore_ascii_case(wanted))
                })
        })
    }

    fn push(
        &mut self,
        kind: &'static str,
        summary: String,
        detail: Vec<Detail>,
        message_id: usize,
    ) {
        self.events.push(Event {
            kind,
            summary,
            detail,
            message_id,
            is_final: matches!(kind, "keep" | "discard" | "reject"),
            after_error: self.after_error,
        });
    }
}

impl<'x> Handler<'x> for Recorder<'_> {
    fn mailbox_exists(
        &mut self,
        _: &Context<'x>,
        mailboxes: &[Mailbox<'_>],
        special_use: &[&str],
    ) -> Reply<bool> {
        let exists = if mailboxes.is_empty() {
            special_use.iter().all(|wanted| {
                self.settings.mailboxes.iter().any(|mailbox| {
                    mailbox
                        .special_use
                        .iter()
                        .any(|have| have.eq_ignore_ascii_case(wanted))
                })
            })
        } else {
            mailboxes.iter().all(|mailbox| match mailbox {
                Mailbox::Name(name) => self.has_mailbox(name, special_use),
                Mailbox::Id(_) => false,
            })
        };
        Reply::Ready(exists)
    }

    fn list_contains(
        &mut self,
        _: &Context<'x>,
        lists: &[&str],
        values: &[&str],
        match_as: MatchAs,
    ) -> Reply<bool> {
        let found = self
            .settings
            .lists
            .iter()
            .filter(|list| lists.contains(&list.name.as_str()))
            .flat_map(|list| list.values.iter())
            .any(|entry| {
                values.iter().any(|value| match match_as {
                    MatchAs::Octet => entry == value,
                    MatchAs::Lowercase => entry.eq_ignore_ascii_case(value),
                    MatchAs::Number => {
                        match (entry.trim().parse::<f64>(), value.trim().parse::<f64>()) {
                            (Ok(a), Ok(b)) => a == b,
                            _ => false,
                        }
                    }
                })
            });
        Reply::Ready(found)
    }

    fn duplicate_id(&mut self, _: &Context<'x>, id: &str, _: u64, _: bool) -> Reply<bool> {
        Reply::Ready(!self.seen_ids.insert(id.to_string()))
    }

    fn action(&mut self, _: &Context<'x>, action: SieveAction<'x>) -> Reply<()> {
        match action {
            SieveAction::Keep { flags, message_id } => {
                self.push(
                    "keep",
                    "Keep the message in INBOX".into(),
                    flags_detail(flags),
                    message_id,
                );
            }
            SieveAction::Discard => {
                self.push(
                    "discard",
                    "Discard the message silently".into(),
                    Vec::new(),
                    0,
                );
            }
            SieveAction::Reject { extended, reason } => {
                let mut detail = vec![Detail::new("reason", reason)];
                if extended {
                    detail.push(Detail::new("type", "ereject (SMTP level)"));
                }
                self.push("reject", "Reject the message".into(), detail, 0);
            }
            SieveAction::FileInto {
                folder,
                flags,
                mailbox_id,
                special_use,
                create,
                message_id,
            } => {
                let mut detail = flags_detail(flags);
                detail.extend(mailbox_id.map(|id| Detail::new("mailbox id", id)));
                detail
                    .extend(special_use.map(|special_use| Detail::new("special use", special_use)));
                if create {
                    detail.push(Detail::new("create", "if missing"));
                }
                if !self.has_mailbox(folder, &[]) && !create {
                    detail.push(Detail::new("warning", "mailbox not in settings"));
                }
                self.push(
                    "fileinto",
                    format!("File into {folder}"),
                    detail,
                    message_id,
                );
            }
            SieveAction::SendMessage {
                source,
                recipient,
                notify,
                return_of_content,
                by_time,
                message_id,
            } => {
                let to = match &recipient {
                    Recipient::Address(address) => (*address).to_string(),
                    Recipient::List(list) => format!("list {list}"),
                    Recipient::Group(group) => group.join(", "),
                };
                let (kind, summary) = match source {
                    MessageSource::Redirect => ("redirect", format!("Redirect to {to}")),
                    MessageSource::Vacation => {
                        ("vacation", format!("Send a vacation reply to {to}"))
                    }
                    MessageSource::Notification => {
                        ("notification", format!("Send a notification to {to}"))
                    }
                };
                let mut detail = Vec::new();
                if let Recipient::Group(group) = &recipient
                    && group.len() > 1
                {
                    detail.push(Detail::new("recipients", group.len().to_string()));
                }
                match notify {
                    Notify::Never if kind == "redirect" => {
                        detail.push(Detail::new("dsn notify", "never"))
                    }
                    Notify::Never => (),
                    Notify::Items(items) => detail.push(Detail::new(
                        "dsn notify",
                        items
                            .iter()
                            .map(|item| match item {
                                NotifyItem::Success => "success",
                                NotifyItem::Failure => "failure",
                                NotifyItem::Delay => "delay",
                            })
                            .collect::<Vec<_>>()
                            .join(", "),
                    )),
                    Notify::Default => (),
                }
                match return_of_content {
                    Ret::Full => detail.push(Detail::new("dsn ret", "full")),
                    Ret::Hdrs => detail.push(Detail::new("dsn ret", "headers")),
                    Ret::Default => (),
                }
                match by_time {
                    ByTime::Relative {
                        rlimit,
                        mode,
                        trace,
                    } => {
                        detail.push(Detail::new("deliver within", format!("{rlimit}s")));
                        push_by_mode(&mut detail, mode, trace);
                    }
                    ByTime::Absolute {
                        alimit,
                        mode,
                        trace,
                    } => {
                        detail.push(Detail::new("deliver by", alimit.to_string()));
                        push_by_mode(&mut detail, mode, trace);
                    }
                    ByTime::None => (),
                }
                self.push(kind, summary, detail, message_id);
            }
            SieveAction::Notify {
                from,
                importance,
                options,
                message,
                method,
            } => {
                let mut detail = Vec::new();
                detail.extend(from.map(|from| Detail::new("from", from)));
                match importance {
                    Importance::High => detail.push(Detail::new("importance", "high")),
                    Importance::Low => detail.push(Detail::new("importance", "low")),
                    Importance::Normal => (),
                }
                if !message.is_empty() {
                    detail.push(Detail::new("message", message));
                }
                if !options.is_empty() {
                    detail.push(Detail::new("options", options.join(", ")));
                }
                self.push("notify", format!("Notify {method}"), detail, 0);
            }
            SieveAction::SetEnvelope { envelope, value } => {
                let field = match envelope {
                    Envelope::From => "from",
                    Envelope::To => "to",
                    Envelope::ByTimeAbsolute => "bytimeabsolute",
                    Envelope::ByTimeRelative => "bytimerelative",
                    Envelope::ByMode => "bymode",
                    Envelope::ByTrace => "bytrace",
                    Envelope::Notify => "notify",
                    Envelope::Orcpt => "orcpt",
                    Envelope::Ret => "ret",
                    Envelope::Envid => "envid",
                };
                self.push(
                    "envelope",
                    format!("Set envelope {field} to {value}"),
                    Vec::new(),
                    0,
                );
            }
            SieveAction::CreatedMessage {
                message_id,
                message,
            } => {
                self.push(
                    "created",
                    format!("Generated message #{message_id}"),
                    vec![Detail::new("size", format!("{} bytes", message.len()))],
                    message_id,
                );
                self.messages
                    .push(OutputMessage::parse(message_id, &message));
            }
        }
        Reply::Ready(())
    }
}

fn flags_detail(flags: &[&str]) -> Vec<Detail> {
    if flags.is_empty() {
        Vec::new()
    } else {
        vec![Detail::new("flags", flags.join(" "))]
    }
}

fn push_by_mode(detail: &mut Vec<Detail>, mode: ByMode, trace: bool) {
    match mode {
        ByMode::Notify => detail.push(Detail::new("by mode", "notify")),
        ByMode::Return => detail.push(Detail::new("by mode", "return")),
        ByMode::Default => (),
    }
    if trace {
        detail.push(Detail::new("by trace", "yes"));
    }
}
