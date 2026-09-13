/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use serde::{Deserialize, Serialize};
use sieve::{
    Compiler, Context, FunctionMap, Metadata, Runtime,
    compiler::grammar::{Capability, Comparator},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub capabilities: Vec<String>,
    pub no_capability_check: bool,

    pub envelope_from: String,
    pub envelope_to: Vec<String>,
    pub envelope_id: String,
    pub user_address: String,
    pub user_full_name: String,
    pub local_hostname: String,
    pub current_time: Option<i64>,

    pub mailboxes: Vec<MailboxSetting>,
    pub lists: Vec<ListSetting>,
    pub environment: Vec<KeyValue>,
    pub global_variables: Vec<KeyValue>,
    pub metadata: Vec<MetadataSetting>,
    pub protected_headers: Vec<String>,
    pub valid_notification_uris: Vec<String>,

    pub spam_score: u32,
    pub virus_score: u32,

    pub vacation_default_subject: String,
    pub vacation_subject_prefix: String,
    pub vacation_use_orig_rcpt: bool,
    pub default_vacation_expiry: u64,
    pub default_duplicate_expiry: u64,

    pub cpu_limit: usize,
    pub memory_limit: usize,
    pub max_redirects: usize,
    pub max_out_messages: usize,
    pub max_nested_includes: usize,
    pub max_variable_size: usize,
    pub max_header_size: usize,
    pub max_received_headers: usize,

    pub max_script_size: usize,
    pub max_string_size: usize,
    pub max_variable_name_size: usize,
    pub max_nested_blocks: usize,
    pub max_nested_tests: usize,
    pub max_nested_foreverypart: usize,
    pub max_match_variables: usize,
    pub max_local_variables: usize,
    pub max_includes: usize,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MailboxSetting {
    pub name: String,
    pub special_use: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ListSetting {
    pub name: String,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct KeyValue {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MetadataSetting {
    pub mailbox: String,
    pub annotation: String,
    pub value: String,
}

const KIB: usize = 1024;
const MIB: usize = 1024 * KIB;
const DAY: u64 = 86400;

pub fn all_capabilities() -> impl Iterator<Item = Capability> {
    Capability::all()
        .iter()
        .filter(|capability| !matches!(capability, Capability::Comparator(Comparator::Elbonia)))
        .cloned()
        .chain([Capability::While, Capability::Expressions])
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            capabilities: all_capabilities().map(|c| c.to_string()).collect(),
            no_capability_check: false,

            envelope_from: String::new(),
            envelope_to: Vec::new(),
            envelope_id: String::new(),
            user_address: "jane@example.org".into(),
            user_full_name: "Jane Doe".into(),
            local_hostname: "mx.example.org".into(),
            current_time: None,

            mailboxes: [
                ("INBOX", None),
                ("Drafts", Some("\\Drafts")),
                ("Sent", Some("\\Sent")),
                ("Junk", Some("\\Junk")),
                ("Trash", Some("\\Trash")),
                ("Archive", Some("\\Archive")),
            ]
            .into_iter()
            .map(|(name, special_use)| MailboxSetting {
                name: name.into(),
                special_use: special_use.into_iter().map(Into::into).collect(),
            })
            .collect(),
            lists: vec![ListSetting {
                name: "addrbook:default".into(),
                values: vec!["boss@example.org".into(), "friend@example.net".into()],
            }],
            environment: [
                ("domain", "example.org"),
                ("host", "mx.example.org"),
                ("location", "MDA"),
                ("phase", "during"),
                ("remote-host", "mail.example.com"),
                ("remote-ip", "192.0.2.25"),
                ("vnd.stalwart.default_mailbox", "INBOX"),
                ("vnd.stalwart.username", "jane"),
            ]
            .into_iter()
            .map(|(name, value)| KeyValue {
                name: name.into(),
                value: value.into(),
            })
            .collect(),
            global_variables: Vec::new(),
            metadata: vec![MetadataSetting {
                mailbox: String::new(),
                annotation: "/private/vendor/vendor.stalwart/playground".into(),
                value: "enabled".into(),
            }],
            protected_headers: vec![
                "Original-Subject".into(),
                "Original-From".into(),
                "Received".into(),
                "Auto-Submitted".into(),
            ],
            valid_notification_uris: vec!["mailto".into(), "xmpp".into(), "tel".into()],

            spam_score: 0,
            virus_score: 0,

            vacation_default_subject: "Automated reply".into(),
            vacation_subject_prefix: "Auto: ".into(),
            vacation_use_orig_rcpt: false,
            default_vacation_expiry: 7 * DAY,
            default_duplicate_expiry: 7 * DAY,

            cpu_limit: 100_000,
            memory_limit: 64 * MIB,
            max_redirects: 10,
            max_out_messages: 20,
            max_nested_includes: 5,
            max_variable_size: 64 * KIB,
            max_header_size: 8 * KIB,
            max_received_headers: 50,

            max_script_size: MIB,
            max_string_size: 64 * KIB,
            max_variable_name_size: 64,
            max_nested_blocks: 32,
            max_nested_tests: 32,
            max_nested_foreverypart: 5,
            max_match_variables: 63,
            max_local_variables: 256,
            max_includes: 10,
        }
    }
}

impl Settings {
    pub fn compiler(&self, functions: &mut FunctionMap) -> Compiler {
        Compiler::new()
            .with_max_script_size(self.max_script_size)
            .with_max_string_size(self.max_string_size)
            .with_max_variable_name_size(self.max_variable_name_size)
            .with_max_nested_blocks(self.max_nested_blocks)
            .with_max_nested_tests(self.max_nested_tests)
            .with_max_nested_foreverypart(self.max_nested_foreverypart)
            .with_max_match_variables(self.max_match_variables)
            .with_max_local_variables(self.max_local_variables)
            .with_max_header_size(self.max_header_size)
            .with_max_includes(self.max_includes)
            .with_no_capability_check(self.no_capability_check)
            .register_functions(functions)
    }

    pub fn runtime(&self, functions: &mut FunctionMap) -> Runtime {
        let mut runtime = Runtime::new()
            .without_capabilities(all_capabilities())
            .with_functions(functions)
            .with_cpu_limit(self.cpu_limit)
            .with_memory_limit(self.memory_limit)
            .with_max_redirects(self.max_redirects)
            .with_max_out_messages(self.max_out_messages)
            .with_max_nested_includes(self.max_nested_includes)
            .with_max_variable_size(self.max_variable_size)
            .with_max_header_size(self.max_header_size)
            .with_max_received_headers(self.max_received_headers)
            .with_local_hostname(self.local_hostname.clone())
            .with_vacation_default_subject(self.vacation_default_subject.clone())
            .with_vacation_subject_prefix(self.vacation_subject_prefix.clone())
            .with_vacation_use_orig_rcpt(self.vacation_use_orig_rcpt)
            .with_default_vacation_expiry(self.default_vacation_expiry)
            .with_default_duplicate_expiry(self.default_duplicate_expiry)
            .with_valid_notification_uris(self.valid_notification_uris.iter().cloned())
            .with_valid_ext_lists(self.lists.iter().map(|list| list.name.clone()));

        for capability in &self.capabilities {
            runtime.set_capability(capability.as_str());
        }
        for header in &self.protected_headers {
            runtime.set_protected_header(header.clone());
        }
        for KeyValue { name, value } in &self.environment {
            runtime.set_env_variable(name.clone(), value.clone());
        }
        for entry in &self.metadata {
            let key = if entry.mailbox.is_empty() {
                Metadata::Server {
                    annotation: entry.annotation.clone(),
                }
            } else {
                Metadata::Mailbox {
                    name: entry.mailbox.clone(),
                    annotation: entry.annotation.clone(),
                }
            };
            runtime.set_medatata(key, entry.value.clone());
        }
        runtime
    }

    pub fn apply<'x>(&'x self, ctx: &mut Context<'x>, now: i64) {
        if !self.envelope_from.is_empty() {
            ctx.set_envelope("from", self.envelope_from.as_str());
        } else if let Some(sender) = ctx
            .message()
            .parts
            .first()
            .and_then(|_| ctx.message().return_address())
            .map(str::to_string)
        {
            ctx.set_envelope("from", sender);
        }
        let mut recipients = self
            .envelope_to
            .iter()
            .filter(|to| !to.is_empty())
            .peekable();
        if recipients.peek().is_none() {
            ctx.set_envelope("to", self.user_address.as_str());
        }
        for to in recipients {
            ctx.set_envelope("to", to.as_str());
        }
        if !self.envelope_id.is_empty() {
            ctx.set_envelope("envid", self.envelope_id.as_str());
        }
        if !self.user_address.is_empty() {
            ctx.set_user_address(self.user_address.as_str());
        }
        if !self.user_full_name.is_empty() {
            ctx.set_user_full_name(&self.user_full_name);
        }
        for KeyValue { name, value } in &self.global_variables {
            ctx.set_global_variable(name.clone(), value.as_str());
        }
        ctx.set_spam_status(self.spam_score);
        ctx.set_virus_status(self.virus_score);
        ctx.set_current_time(self.current_time.unwrap_or(now));
    }
}
