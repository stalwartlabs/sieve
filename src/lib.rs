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

//! # sieve
//!
//! [![crates.io](https://img.shields.io/crates/v/sieve-rs)](https://crates.io/crates/sieve-rs)
//! [![build](https://github.com/stalwartlabs/sieve/actions/workflows/rust.yml/badge.svg)](https://github.com/stalwartlabs/sieve/actions/workflows/rust.yml)
//! [![docs.rs](https://img.shields.io/docsrs/sieve-rs)](https://docs.rs/sieve-rs)
//! [![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)
//!
//! _sieve_ is a fast and secure Sieve filter interpreter for Rust that supports all [registered Sieve extensions](https://www.iana.org/assignments/sieve-extensions/sieve-extensions.xhtml).
//!
//! ## Usage Example
//!
//! ```rust
//!     use sieve::{runtime::RuntimeError, Compiler, Event, Input, Runtime};
//!
//!     let text_script = br#"
//!     require ["fileinto", "body", "imap4flags"];
//!     
//!     if body :contains "tps" {
//!         setflag "$tps_reports";
//!     }
//!
//!     if header :matches "List-ID" "*<*@*" {
//!         fileinto "INBOX.lists.${2}"; stop;
//!     }
//!     "#;
//!     let raw_message = r#"From: Sales Mailing List <list-sales@example.org>
//! To: John Doe <jdoe@example.org>
//! List-ID: <sales@example.org>
//! Subject: TPS Reports
//!
//! We're putting new coversheets on all the TPS reports before they go out now.
//! So if you could go ahead and try to remember to do that from now on, that'd be great. All right!
//! "#;
//!
//!     // Compile
//!     let compiler = Compiler::new();
//!     let script = compiler.compile(text_script).unwrap();
//!
//!     // Build runtime
//!     let runtime = Runtime::new();
//!
//!     // Create filter instance
//!     let mut instance = runtime.filter(raw_message.as_bytes());
//!     let mut input = Input::script("my-script", script);
//!     let mut messages: Vec<String> = Vec::new();
//!
//!     // Start event loop
//!     while let Some(result) = instance.run(input) {
//!         match result {
//!             Ok(event) => match event {
//!                 Event::IncludeScript { name, optional } => {
//!                     // NOTE: Just for demonstration purposes, script name needs to be validated first.
//!                     if let Ok(bytes) = std::fs::read(name.as_str()) {
//!                         let script = compiler.compile(&bytes).unwrap();
//!                         input = Input::script(name, script);
//!                     } else if optional {
//!                         input = Input::False;
//!                     } else {
//!                         panic!("Script {} not found.", name);
//!                     }
//!                 }
//!                 Event::MailboxExists { .. } => {
//!                     // Set to true if the mailbox exists
//!                     input = false.into();
//!                 }
//!                 Event::ListContains { .. } => {
//!                     // Set to true if the list(s) contains an entry
//!                     input = false.into();
//!                 }
//!                 Event::DuplicateId { .. } => {
//!                     // Set to true if the ID is duplicate
//!                     input = false.into();
//!                 }
//!                 Event::Plugin { id, arguments } => {
//!                     println!("Script executed plugin {id} with parameters {arguments:?}");
//!                     // Set to true if the script succeeded
//!                     input = false.into();
//!                 }
//!                 Event::SetEnvelope { envelope, value } => {
//!                     println!("Set envelope {envelope:?} to {value:?}");
//!                     input = true.into();
//!                 }
//!
//!                 Event::Keep { flags, message_id } => {
//!                     println!(
//!                         "Keep message '{}' with flags {:?}.",
//!                         if message_id > 0 {
//!                             messages[message_id - 1].as_str()
//!                         } else {
//!                             raw_message
//!                         },
//!                         flags
//!                     );
//!                     input = true.into();
//!                 }
//!                 Event::Discard => {
//!                     println!("Discard message.");
//!                     input = true.into();
//!                 }
//!                 Event::Reject { reason, .. } => {
//!                     println!("Reject message with reason {:?}.", reason);
//!                     input = true.into();
//!                 }
//!                 Event::FileInto {
//!                     folder,
//!                     flags,
//!                     message_id,
//!                     ..
//!                 } => {
//!                     println!(
//!                         "File message '{}' in folder {:?} with flags {:?}.",
//!                         if message_id > 0 {
//!                             messages[message_id - 1].as_str()
//!                         } else {
//!                             raw_message
//!                         },
//!                         folder,
//!                         flags
//!                     );
//!                     input = true.into();
//!                 }
//!                 Event::SendMessage {
//!                     recipient,
//!                     message_id,
//!                     ..
//!                 } => {
//!                     println!(
//!                         "Send message '{}' to {:?}.",
//!                         if message_id > 0 {
//!                             messages[message_id - 1].as_str()
//!                         } else {
//!                             raw_message
//!                         },
//!                         recipient
//!                     );
//!                     input = true.into();
//!                 }
//!                 Event::Notify {
//!                     message, method, ..
//!                 } => {
//!                     println!("Notify URI {:?} with message {:?}", method, message);
//!                     input = true.into();
//!                 }
//!                 Event::CreatedMessage { message, .. } => {
//!                     messages.push(String::from_utf8(message).unwrap());
//!                     input = true.into();
//!                 }
//!
//!                 #[cfg(test)]
//!                 _ => unreachable!(),
//!             },
//!             Err(error) => {
//!                 match error {
//!                     RuntimeError::TooManyIncludes => {
//!                         eprintln!("Too many included scripts.");
//!                     }
//!                     RuntimeError::InvalidInstruction(instruction) => {
//!                         eprintln!(
//!                             "Invalid instruction {:?} found at {}:{}.",
//!                             instruction.name(),
//!                             instruction.line_num(),
//!                             instruction.line_pos()
//!                         );
//!                     }
//!                     RuntimeError::ScriptErrorMessage(message) => {
//!                         eprintln!("Script called the 'error' function with {:?}", message);
//!                     }
//!                     RuntimeError::CapabilityNotAllowed(capability) => {
//!                         eprintln!(
//!                             "Capability {:?} has been disabled by the administrator.",
//!                             capability
//!                         );
//!                     }
//!                     RuntimeError::CapabilityNotSupported(capability) => {
//!                         eprintln!("Capability {:?} not supported.", capability);
//!                     }
//!                     RuntimeError::CPULimitReached => {
//!                         eprintln!("Script exceeded the configured CPU limit.");
//!                     }
//!                 }
//!                 input = true.into();
//!             }
//!         }
//!     }
//! ```
//!
//! ## Testing and Fuzzing
//!
//! To run the testsuite:
//!
//! ```bash
//!  $ cargo test --all-features
//! ```
//!
//! To fuzz the library with `cargo-fuzz`:
//!
//! ```bash
//!  $ cargo +nightly fuzz run mail_parser
//! ```
//!
//! ## Conformed RFCs
//!
//! - [RFC 5228 - Sieve: An Email Filtering Language](https://datatracker.ietf.org/doc/html/rfc5228)
//! - [RFC 3894 - Copying Without Side Effects](https://datatracker.ietf.org/doc/html/rfc3894)
//! - [RFC 5173 - Body Extension](https://datatracker.ietf.org/doc/html/rfc5173)
//! - [RFC 5183 - Environment Extension](https://datatracker.ietf.org/doc/html/rfc5183)
//! - [RFC 5229 - Variables Extension](https://datatracker.ietf.org/doc/html/rfc5229)
//! - [RFC 5230 - Vacation Extension](https://datatracker.ietf.org/doc/html/rfc5230)
//! - [RFC 5231 - Relational Extension](https://datatracker.ietf.org/doc/html/rfc5231)
//! - [RFC 5232 - Imap4flags Extension](https://datatracker.ietf.org/doc/html/rfc5232)
//! - [RFC 5233 - Subaddress Extension](https://datatracker.ietf.org/doc/html/rfc5233)
//! - [RFC 5235 - Spamtest and Virustest Extensions](https://datatracker.ietf.org/doc/html/rfc5235)
//! - [RFC 5260 - Date and Index Extensions](https://datatracker.ietf.org/doc/html/rfc5260)
//! - [RFC 5293 - Editheader Extension](https://datatracker.ietf.org/doc/html/rfc5293)
//! - [RFC 5429 - Reject and Extended Reject Extensions](https://datatracker.ietf.org/doc/html/rfc5429)
//! - [RFC 5435 - Extension for Notifications](https://datatracker.ietf.org/doc/html/rfc5435)
//! - [RFC 5463 - Ihave Extension](https://datatracker.ietf.org/doc/html/rfc5463)
//! - [RFC 5490 - Extensions for Checking Mailbox Status and Accessing Mailbox Metadata](https://datatracker.ietf.org/doc/html/rfc5490)
//! - [RFC 5703 - MIME Part Tests, Iteration, Extraction, Replacement, and Enclosure](https://datatracker.ietf.org/doc/html/rfc5703)
//! - [RFC 6009 - Delivery Status Notifications and Deliver-By Extensions](https://datatracker.ietf.org/doc/html/rfc6009)
//! - [RFC 6131 - Sieve Vacation Extension: "Seconds" Parameter](https://datatracker.ietf.org/doc/html/rfc6131)
//! - [RFC 6134 - Externally Stored Lists](https://datatracker.ietf.org/doc/html/rfc6134)
//! - [RFC 6558 - Converting Messages before Delivery](https://datatracker.ietf.org/doc/html/rfc6558)
//! - [RFC 6609 - Include Extension](https://datatracker.ietf.org/doc/html/rfc6609)
//! - [RFC 7352 - Detecting Duplicate Deliveries](https://datatracker.ietf.org/doc/html/rfc7352)
//! - [RFC 8579 - Delivering to Special-Use Mailboxes](https://datatracker.ietf.org/doc/html/rfc8579)
//! - [RFC 8580 - File Carbon Copy (FCC)](https://datatracker.ietf.org/doc/html/rfc8580)
//! - [RFC 9042 - Delivery by MAILBOXID](https://datatracker.ietf.org/doc/html/rfc9042)
//! - [REGEX-01 - Regular Expression Extension (draft-ietf-sieve-regex-01)](https://www.ietf.org/archive/id/draft-ietf-sieve-regex-01.html)
//!
//! ## License
//!
//! Licensed under the terms of the [GNU Affero General Public License](https://www.gnu.org/licenses/agpl-3.0.en.html) as published by
//! the Free Software Foundation, either version 3 of the License, or (at your option) any later version.
//! See [LICENSE](LICENSE) for more details.
//!
//! You can be released from the requirements of the AGPLv3 license by purchasing
//! a commercial license. Please contact licensing@stalw.art for more details.
//!   
//! ## Copyright
//!
//! Copyright (C) 2020-2023, Stalwart Labs Ltd.
//!

use std::{borrow::Cow, iter::Enumerate, sync::Arc, vec::IntoIter};

use ahash::{AHashMap, AHashSet};
use compiler::{
    grammar::{
        actions::action_redirect::{ByTime, Notify, Ret},
        instruction::Instruction,
        Capability,
    },
    Number, Regex, VariableType,
};
use mail_parser::{HeaderName, Message};
use runtime::{context::ScriptStack, Variable};
use serde::{Deserialize, Serialize};

pub mod compiler;
pub mod runtime;

pub(crate) const MAX_MATCH_VARIABLES: usize = 63;
pub(crate) const MAX_LOCAL_VARIABLES: usize = 256;

#[derive(Debug, Clone, Eq, PartialEq, Serialize, Deserialize)]
pub struct Sieve {
    instructions: Vec<Instruction>,
    num_vars: usize,
    num_match_vars: usize,
}

pub struct Compiler {
    // Settings
    pub(crate) max_script_size: usize,
    pub(crate) max_string_size: usize,
    pub(crate) max_variable_name_size: usize,
    pub(crate) max_nested_blocks: usize,
    pub(crate) max_nested_tests: usize,
    pub(crate) max_nested_foreverypart: usize,
    pub(crate) max_match_variables: usize,
    pub(crate) max_local_variables: usize,
    pub(crate) max_header_size: usize,
    pub(crate) max_includes: usize,

    // Plugins
    pub(crate) plugins: AHashMap<String, PluginSchema>,
    pub(crate) functions: AHashMap<String, (u32, u32)>,
}

pub type Function = fn(Vec<Variable<'_>>) -> Variable<'static>;

#[derive(Default, Clone)]
pub struct FunctionMap {
    pub(crate) map: AHashMap<String, (u32, u32)>,
    pub(crate) functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub(crate) allowed_capabilities: AHashSet<Capability>,
    pub(crate) valid_notification_uris: AHashSet<Cow<'static, str>>,
    pub(crate) valid_ext_lists: AHashSet<Cow<'static, str>>,
    pub(crate) protected_headers: Vec<HeaderName<'static>>,
    pub(crate) environment: AHashMap<Cow<'static, str>, Variable<'static>>,
    pub(crate) metadata: Vec<(Metadata<String>, Cow<'static, str>)>,
    pub(crate) include_scripts: AHashMap<String, Arc<Sieve>>,
    pub(crate) local_hostname: Cow<'static, str>,
    pub(crate) functions: Vec<Function>,

    pub(crate) max_nested_includes: usize,
    pub(crate) cpu_limit: usize,
    pub(crate) max_variable_size: usize,
    pub(crate) max_redirects: usize,
    pub(crate) max_received_headers: usize,
    pub(crate) max_header_size: usize,
    pub(crate) max_out_messages: usize,

    pub(crate) default_vacation_expiry: u64,
    pub(crate) default_duplicate_expiry: u64,

    pub(crate) vacation_use_orig_rcpt: bool,
    pub(crate) vacation_default_subject: Cow<'static, str>,
    pub(crate) vacation_subject_prefix: Cow<'static, str>,
}

#[derive(Clone, Debug)]
pub struct Context<'x> {
    pub(crate) runtime: &'x Runtime,
    pub(crate) user_address: Cow<'x, str>,
    pub(crate) user_full_name: Cow<'x, str>,
    pub(crate) current_time: i64,

    pub(crate) message: Message<'x>,
    pub(crate) message_size: usize,
    pub(crate) envelope: Vec<(Envelope, Variable<'x>)>,
    pub(crate) metadata: Vec<(Metadata<String>, Cow<'x, str>)>,

    pub(crate) part: usize,
    pub(crate) part_iter: IntoIter<usize>,
    pub(crate) part_iter_stack: Vec<(usize, IntoIter<usize>)>,

    pub(crate) line_iter: Enumerate<IntoIter<Variable<'static>>>,

    pub(crate) spam_status: SpamStatus,
    pub(crate) virus_status: VirusStatus,

    pub(crate) pos: usize,
    pub(crate) test_result: bool,
    pub(crate) script_cache: AHashMap<Script, Arc<Sieve>>,
    pub(crate) script_stack: Vec<ScriptStack>,
    pub(crate) vars_global: AHashMap<Cow<'static, str>, Variable<'static>>,
    pub(crate) vars_env: AHashMap<Cow<'static, str>, Variable<'x>>,
    pub(crate) vars_local: Vec<Variable<'static>>,
    pub(crate) vars_match: Vec<Variable<'static>>,

    pub(crate) queued_events: IntoIter<Event>,
    pub(crate) final_event: Option<Event>,
    pub(crate) last_message_id: usize,
    pub(crate) main_message_id: usize,

    pub(crate) has_changes: bool,
    pub(crate) num_redirects: usize,
    pub(crate) num_instructions: usize,
    pub(crate) num_out_messages: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Script {
    Personal(String),
    Global(String),
}

pub struct PluginSchema {
    pub id: ExternalId,
    pub tags: AHashMap<String, PluginSchemaTag>,
    pub arguments: Vec<PluginSchemaArgument>,
}

pub enum PluginSchemaArgument {
    Text,
    Number,
    Regex,
    Variable,
    Array(Box<PluginSchemaArgument>),
}

pub struct PluginSchemaTag {
    pub id: ExternalId,
    pub argument: Option<PluginSchemaArgument>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Envelope {
    From,
    To,
    ByTimeAbsolute,
    ByTimeRelative,
    ByMode,
    ByTrace,
    Notify,
    Orcpt,
    Ret,
    Envid,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
pub enum Metadata<T> {
    Server { annotation: T },
    Mailbox { name: T, annotation: T },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event {
    IncludeScript {
        name: Script,
        optional: bool,
    },
    MailboxExists {
        mailboxes: Vec<Mailbox>,
        special_use: Vec<String>,
    },
    ListContains {
        lists: Vec<String>,
        values: Vec<String>,
        match_as: MatchAs,
    },
    DuplicateId {
        id: String,
        expiry: u64,
        last: bool,
    },
    Plugin {
        id: ExternalId,
        arguments: Vec<PluginArgument<String, Number>>,
    },
    SetEnvelope {
        envelope: Envelope,
        value: String,
    },

    // Actions
    Keep {
        flags: Vec<String>,
        message_id: usize,
    },
    Discard,
    Reject {
        extended: bool,
        reason: String,
    },
    FileInto {
        folder: String,
        flags: Vec<String>,
        mailbox_id: Option<String>,
        special_use: Option<String>,
        create: bool,
        message_id: usize,
    },
    SendMessage {
        recipient: Recipient,
        notify: Notify,
        return_of_content: Ret,
        by_time: ByTime<i64>,
        message_id: usize,
    },
    Notify {
        from: Option<String>,
        importance: Importance,
        options: Vec<String>,
        message: String,
        method: String,
    },
    CreatedMessage {
        message_id: usize,
        message: Vec<u8>,
    },
}

pub type ExternalId = u32;

#[derive(Clone, PartialEq, Eq, Debug, Serialize, Deserialize)]
pub enum PluginArgument<T, N> {
    Tag(ExternalId),
    Text(T),
    Number(N),
    Regex(Regex),
    Variable(VariableType),
    Array(Vec<PluginArgument<T, N>>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Hash)]
pub(crate) struct FileCarbonCopy<T> {
    pub mailbox: T,
    pub mailbox_id: Option<T>,
    pub create: bool,
    pub flags: Vec<T>,
    pub special_use: Option<T>,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Importance {
    High,
    Normal,
    Low,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum MatchAs {
    Octet,
    Lowercase,
    Number,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Recipient {
    Address(String),
    List(String),
    Group(Vec<String>),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Input {
    True,
    False,
    Script { name: Script, script: Arc<Sieve> },
    Variables { list: Vec<SetVariable> },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct SetVariable {
    pub name: VariableType,
    pub value: Variable<'static>,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Mailbox {
    Name(String),
    Id(String),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpamStatus {
    Unknown,
    Ham,
    MaybeSpam(f64),
    Spam,
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum VirusStatus {
    Unknown,
    Clean,
    Replaced,
    Cured,
    MaybeVirus,
    Virus,
}
