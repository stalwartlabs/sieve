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
    pub(crate) functions: AHashMap<String, usize>,
}

pub type Function = fn(Variable<'_>) -> Variable<'static>;

#[derive(Default, Clone)]
pub struct FunctionMap {
    pub(crate) map: AHashMap<String, usize>,
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
    #[cfg(test)]
    pub(crate) runtime: Runtime,
    #[cfg(not(test))]
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

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use ahash::{AHashMap, AHashSet};
    use mail_parser::{
        parsers::MessageStream, Encoding, HeaderValue, Message, MessageParser, MessagePart,
        PartType,
    };

    use crate::{
        compiler::grammar::Capability, runtime::actions::action_mime::reset_test_boundary,
        Compiler, Envelope, Event, FunctionMap, Input, Mailbox, PluginArgument, Recipient, Runtime,
        SpamStatus, VirusStatus,
    };

    #[test]
    fn test_suite() {
        let mut tests = Vec::new();
        let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        path.push("tests");

        read_dir(path, &mut tests);

        for test in tests {
            /*if !test
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .contains("expressions")
            {
                continue;
            }*/
            println!("===== {} =====", test.display());
            run_test(&test);
        }
    }

    fn read_dir(path: PathBuf, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap().path();
            if entry.is_dir() {
                read_dir(entry, files);
            } else if entry
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .eq("svtest")
            {
                files.push(entry);
            }
        }
    }

    fn run_test(script_path: &Path) {
        let mut fnc_map = FunctionMap::new()
            .with_function("trim", |v| v.to_cow().trim().to_string().into())
            .with_function("len", |v| v.to_cow().len().into())
            .with_function("to_lowercase", |v| {
                v.to_cow().to_lowercase().to_string().into()
            })
            .with_function("to_uppercase", |v| {
                v.to_cow().to_uppercase().to_string().into()
            })
            .with_function("is_uppercase", |v| {
                v.to_cow()
                    .as_ref()
                    .chars()
                    .filter(|c| c.is_alphabetic())
                    .all(|c| c.is_uppercase())
                    .into()
            })
            .with_function("char_count", |v| v.to_cow().as_ref().chars().count().into());
        let mut compiler = Compiler::new()
            .with_max_string_size(10240)
            .register_functions(&mut fnc_map);

        // Register extensions
        compiler
            .register_plugin("execute")
            .with_tag("query")
            .with_tag("binary")
            .with_string_argument()
            .with_string_array_argument();

        let mut ancestors = script_path.ancestors();
        ancestors.next();
        let base_path = ancestors.next().unwrap();
        let script = compiler
            .compile(&add_crlf(&fs::read(script_path).unwrap()))
            .unwrap();

        let mut input = Input::script("", script);
        let mut current_test = String::new();
        let mut raw_message_: Option<Vec<u8>> = None;
        let mut prev_state = None;
        let mut mailboxes = Vec::new();
        let mut lists: AHashMap<String, AHashSet<String>> = AHashMap::new();
        let mut duplicated_ids = AHashSet::new();
        let mut actions = Vec::new();

        'outer: loop {
            let runtime = Runtime::new()
                .with_protected_header("Auto-Submitted")
                .with_protected_header("Received")
                .with_valid_notification_uri("mailto")
                .with_max_out_messages(100)
                .with_capability(Capability::Plugins)
                .with_capability(Capability::ForEveryLine)
                .with_functions(&mut fnc_map.clone());
            let mut instance = runtime.filter(b"");
            let raw_message = raw_message_.take().unwrap_or_default();
            instance.message =
                MessageParser::new()
                    .parse(&raw_message)
                    .unwrap_or_else(|| Message {
                        html_body: vec![],
                        text_body: vec![],
                        attachments: vec![],
                        parts: vec![MessagePart {
                            headers: vec![],
                            is_encoding_problem: false,
                            body: PartType::Text("".into()),
                            encoding: Encoding::None,
                            offset_header: 0,
                            offset_body: 0,
                            offset_end: 0,
                        }],
                        raw_message: b""[..].into(),
                    });
            instance.message_size = raw_message.len();
            if let Some((pos, script_cache, script_stack, vars_global, vars_local, vars_match)) =
                prev_state.take()
            {
                instance.pos = pos;
                instance.script_cache = script_cache;
                instance.script_stack = script_stack;
                instance.vars_global = vars_global;
                instance.vars_local = vars_local;
                instance.vars_match = vars_match;
            }
            instance.set_env_variable("vnd.stalwart.default_mailbox", "INBOX");
            instance.set_env_variable("vnd.stalwart.username", "john.doe");
            instance.set_user_address("MAILER-DAEMON");
            if let Some(addr) = instance
                .message
                .from()
                .and_then(|a| a.first())
                .and_then(|a| a.address.as_ref())
            {
                instance.set_envelope(Envelope::From, addr.to_string());
            }
            if let Some(addr) = instance
                .message
                .to()
                .and_then(|a| a.first())
                .and_then(|a| a.address.as_ref())
            {
                instance.set_envelope(Envelope::To, addr.to_string());
            }

            while let Some(event) = instance.run(input) {
                match event.unwrap() {
                    Event::IncludeScript { name, optional } => {
                        let mut include_path = PathBuf::from(base_path);
                        include_path.push(if matches!(name, crate::Script::Personal(_)) {
                            "included"
                        } else {
                            "included-global"
                        });
                        include_path.push(format!("{name}.sieve"));

                        if let Ok(bytes) = fs::read(include_path.as_path()) {
                            let script = compiler.compile(&add_crlf(&bytes)).unwrap();
                            input = Input::script(name, script);
                        } else if optional {
                            input = Input::False;
                        } else {
                            panic!("Script {} not found.", include_path.display());
                        }
                    }
                    Event::MailboxExists {
                        mailboxes: mailboxes_,
                        special_use,
                    } => {
                        for action in &actions {
                            if let Event::FileInto { folder, create, .. } = action {
                                if *create && !mailboxes.contains(folder) {
                                    mailboxes.push(folder.to_string());
                                }
                            }
                        }
                        input = (special_use.is_empty()
                            && mailboxes_.iter().all(|n| {
                                if let Mailbox::Name(n) = n {
                                    mailboxes.contains(n)
                                } else {
                                    false
                                }
                            }))
                        .into();
                    }
                    Event::ListContains {
                        lists: lists_,
                        values,
                        ..
                    } => {
                        let mut result = false;
                        'list: for list in &lists_ {
                            if let Some(list) = lists.get(list) {
                                for value in &values {
                                    if list.contains(value) {
                                        result = true;
                                        break 'list;
                                    }
                                }
                            }
                        }

                        input = result.into();
                    }
                    Event::DuplicateId { id, .. } => {
                        input = duplicated_ids.contains(&id).into();
                    }
                    Event::Plugin { id, arguments } => {
                        if id == u32::MAX {
                            // Test functions
                            input = Input::True;
                            let mut arguments = arguments.into_iter();
                            let command = arguments.next().unwrap().unwrap_string().unwrap();
                            let mut params = arguments
                                .map(|arg| arg.unwrap_string().unwrap())
                                .collect::<Vec<_>>();

                            match command.as_str() {
                                "test" => {
                                    current_test = params.pop().unwrap();
                                    println!("Running test '{current_test}'...");
                                }
                                "test_set" => {
                                    let mut params = params.into_iter();
                                    let target = params.next().expect("test_set parameter");
                                    if target == "message" {
                                        let value = params.next().unwrap();
                                        raw_message_ = if value.eq_ignore_ascii_case(":smtp") {
                                            let mut message = None;
                                            for action in actions.iter().rev() {
                                                if let Event::SendMessage { message_id, .. } =
                                                    action
                                                {
                                                    let message_ = actions
                                                        .iter()
                                                        .find_map(|item| {
                                                            if let Event::CreatedMessage {
                                                                message_id: message_id_,
                                                                message,
                                                            } = item
                                                            {
                                                                if message_id == message_id_ {
                                                                    return Some(message);
                                                                }
                                                            }
                                                            None
                                                        })
                                                        .unwrap();
                                                    /*println!(
                                                        "<[{}]>",
                                                        std::str::from_utf8(message_).unwrap()
                                                    );*/
                                                    message = message_.into();
                                                    break;
                                                }
                                            }
                                            message.expect("No SMTP message found").to_vec().into()
                                        } else {
                                            value.into_bytes().into()
                                        };
                                        prev_state = (
                                            instance.pos,
                                            instance.script_cache,
                                            instance.script_stack,
                                            instance.vars_global,
                                            instance.vars_local,
                                            instance.vars_match,
                                        )
                                            .into();

                                        continue 'outer;
                                    } else if let Some(envelope) = target.strip_prefix("envelope.")
                                    {
                                        let envelope =
                                            Envelope::try_from(envelope.to_string()).unwrap();
                                        instance.envelope.retain(|(e, _)| e != &envelope);
                                        instance.set_envelope(envelope, params.next().unwrap());
                                    } else if target == "currentdate" {
                                        let bytes = params.next().unwrap().into_bytes();
                                        if let HeaderValue::DateTime(dt) =
                                            MessageStream::new(&bytes).parse_date()
                                        {
                                            instance.current_time = dt.to_timestamp();
                                        } else {
                                            panic!("Invalid currentdate");
                                        }
                                    } else {
                                        panic!("test_set {target} not implemented.");
                                    }
                                }
                                "test_message" => {
                                    let mut params = params.into_iter();
                                    input = match params.next().unwrap().as_str() {
                                    ":folder" => {
                                        let folder_name = params.next().expect("test_message folder name");
                                        matches!(&instance.final_event, Some(Event::Keep { .. })) ||
                                            actions.iter().any(|a| if !folder_name.eq_ignore_ascii_case("INBOX") {
                                                matches!(a, Event::FileInto { folder, .. } if folder == &folder_name )
                                            } else {
                                                matches!(a, Event::Keep { .. })
                                            })
                                    }
                                    ":smtp" => {
                                        actions.iter().any(|a| matches!(a, Event::SendMessage { .. } ))
                                    }
                                    param => panic!("Invalid test_message param '{param}'" ),
                                }.into();
                                }
                                "test_assert_message" => {
                                    let expected_message =
                                        params.first().expect("test_set parameter");
                                    let built_message = instance.build_message();
                                    if expected_message.as_bytes() != built_message {
                                        //fs::write("_deleteme.json", serde_json::to_string_pretty(&Message::parse(&built_message).unwrap()).unwrap()).unwrap();
                                        print!("<[");
                                        print!("{}", String::from_utf8(built_message).unwrap());
                                        println!("]>");
                                        panic!("Message built incorrectly at '{current_test}'");
                                    }
                                }
                                "test_config_set" => {
                                    let mut params = params.into_iter();
                                    let name = params.next().unwrap();
                                    let value = params.next().expect("test_config_set value");

                                    match name.as_str() {
                                        "sieve_editheader_protected"
                                        | "sieve_editheader_forbid_add"
                                        | "sieve_editheader_forbid_delete" => {
                                            if !value.is_empty() {
                                                for header_name in value.split(' ') {
                                                    instance.runtime.set_protected_header(
                                                        header_name.to_string(),
                                                    );
                                                }
                                            } else {
                                                instance.runtime.protected_headers.clear();
                                            }
                                        }
                                        "sieve_variables_max_variable_size" => {
                                            instance
                                                .runtime
                                                .set_max_variable_size(value.parse().unwrap());
                                        }
                                        "sieve_valid_ext_list" => {
                                            instance.runtime.set_valid_ext_list(value);
                                        }
                                        "sieve_ext_list_item" => {
                                            lists
                                                .entry(value)
                                                .or_insert_with(AHashSet::new)
                                                .insert(params.next().expect("list item value"));
                                        }
                                        "sieve_duplicated_id" => {
                                            duplicated_ids.insert(value);
                                        }
                                        "sieve_user_email" => {
                                            instance.set_user_address(value);
                                        }
                                        "sieve_vacation_use_original_recipient" => {
                                            instance.runtime.set_vacation_use_orig_rcpt(
                                                value.eq_ignore_ascii_case("yes"),
                                            );
                                        }
                                        "sieve_vacation_default_subject" => {
                                            instance.runtime.set_vacation_default_subject(value);
                                        }
                                        "sieve_vacation_default_subject_template" => {
                                            instance.runtime.set_vacation_subject_prefix(value);
                                        }
                                        "sieve_spam_status" => {
                                            instance.set_spam_status(SpamStatus::from_number(
                                                value.parse().unwrap(),
                                            ));
                                        }
                                        "sieve_spam_status_plus" => {
                                            instance.set_spam_status(
                                                match value.parse::<u32>().unwrap() {
                                                    0 => SpamStatus::Unknown,
                                                    100.. => SpamStatus::Spam,
                                                    n => SpamStatus::MaybeSpam((n as f64) / 100.0),
                                                },
                                            );
                                        }
                                        "sieve_virus_status" => {
                                            instance.set_virus_status(VirusStatus::from_number(
                                                value.parse().unwrap(),
                                            ));
                                        }
                                        "sieve_editheader_max_header_size" => {
                                            let mhs = if !value.is_empty() {
                                                value.parse::<usize>().unwrap()
                                            } else {
                                                1024
                                            };
                                            instance.runtime.set_max_header_size(mhs);
                                            compiler.set_max_header_size(mhs);
                                        }
                                        "sieve_include_max_includes" => {
                                            compiler.set_max_includes(if !value.is_empty() {
                                                value.parse::<usize>().unwrap()
                                            } else {
                                                3
                                            });
                                        }
                                        "sieve_include_max_nesting_depth" => {
                                            compiler.set_max_nested_blocks(if !value.is_empty() {
                                                value.parse::<usize>().unwrap()
                                            } else {
                                                3
                                            });
                                        }
                                        param => panic!("Invalid test_config_set param '{param}'"),
                                    }
                                }
                                "test_result_execute" => {
                                    input =
                                        (matches!(&instance.final_event, Some(Event::Keep { .. }))
                                            || actions.iter().any(|a| {
                                                matches!(
                                                    a,
                                                    Event::Keep { .. }
                                                        | Event::FileInto { .. }
                                                        | Event::SendMessage { .. }
                                                )
                                            }))
                                        .into();
                                }
                                "test_result_action" => {
                                    let param =
                                        params.first().expect("test_result_action parameter");
                                    input = if param == "reject" {
                                        (actions.iter().any(|a| matches!(a, Event::Reject { .. })))
                                            .into()
                                    } else if param == "redirect" {
                                        let param = params
                                            .last()
                                            .expect("test_result_action redirect address");
                                        (actions
                                        .iter()
                                        .any(|a| matches!(a, Event::SendMessage { recipient: Recipient::Address(address), .. } if address == param)))
                                    .into()
                                    } else if param == "keep" {
                                        (matches!(&instance.final_event, Some(Event::Keep { .. }))
                                            || actions
                                                .iter()
                                                .any(|a| matches!(a, Event::Keep { .. })))
                                        .into()
                                    } else if param == "send_message" {
                                        (actions
                                            .iter()
                                            .any(|a| matches!(a, Event::SendMessage { .. })))
                                        .into()
                                    } else {
                                        panic!("test_result_action {param} not implemented");
                                    };
                                }
                                "test_result_action_count" => {
                                    input = (actions.len()
                                        == params.first().unwrap().parse::<usize>().unwrap())
                                    .into();
                                }
                                "test_imap_metadata_set" => {
                                    let mut params = params.into_iter();
                                    let first = params.next().expect("metadata parameter");
                                    let (mailbox, annotation) = if first == ":mailbox" {
                                        (
                                            params.next().expect("metadata mailbox name").into(),
                                            params.next().expect("metadata annotation name"),
                                        )
                                    } else {
                                        (None, first)
                                    };
                                    let value = params.next().expect("metadata value");
                                    if let Some(mailbox) = mailbox {
                                        instance.set_medatata((mailbox, annotation), value);
                                    } else {
                                        instance.set_medatata(annotation, value);
                                    }
                                }
                                "test_mailbox_create" => {
                                    mailboxes.push(params.pop().expect("mailbox to create"));
                                }
                                "test_result_reset" => {
                                    actions.clear();
                                    instance.final_event = Event::Keep {
                                        flags: vec![],
                                        message_id: 0,
                                    }
                                    .into();
                                    instance.metadata.clear();
                                    instance.has_changes = false;
                                    instance.num_redirects = 0;
                                    instance.runtime.vacation_use_orig_rcpt = false;
                                    mailboxes.clear();
                                    lists.clear();
                                    reset_test_boundary();
                                }
                                "test_script_compile" => {
                                    let mut include_path = PathBuf::from(base_path);
                                    include_path.push(params.first().unwrap());

                                    if let Ok(bytes) = fs::read(include_path.as_path()) {
                                        let result = compiler.compile(&add_crlf(&bytes));
                                        /*if let Err(err) = &result {
                                            println!("Error: {:?}", err);
                                        }*/
                                        input = result.is_ok().into();
                                    } else {
                                        panic!("Script {} not found.", include_path.display());
                                    }
                                }
                                "test_config_reload" => (),
                                "test_fail" => {
                                    panic!(
                                        "Test '{}' failed: {}",
                                        current_test,
                                        params.pop().unwrap()
                                    );
                                }
                                _ => panic!("Test command {command} not implemented."),
                            }
                        } else {
                            let mut arguments = arguments
                                .into_iter()
                                .filter(|a| !matches!(a, PluginArgument::Tag(_)));
                            let command = arguments.next().unwrap().unwrap_string().unwrap();
                            let arguments =
                                arguments.next().unwrap().unwrap_string_array().unwrap();

                            assert_eq!(arguments, ["param1", "param2"]);
                            input = (if command.eq_ignore_ascii_case("always_succeed") {
                                true
                            } else if command.eq_ignore_ascii_case("always_fail") {
                                false
                            } else {
                                panic!("Unknown command {command}");
                            })
                            .into();
                        }
                    }

                    action => {
                        actions.push(action);
                        input = true.into();
                    }
                }
            }

            return;
        }
    }

    fn add_crlf(bytes: &[u8]) -> Vec<u8> {
        let mut result = Vec::with_capacity(bytes.len());
        let mut last_ch = 0;
        for &ch in bytes {
            if ch == b'\n' && last_ch != b'\r' {
                result.push(b'\r');
            }
            result.push(ch);
            last_ch = ch;
        }
        result
    }
}
