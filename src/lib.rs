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

#![doc = include_str!("../README.md")]

use std::{borrow::Cow, sync::Arc, vec::IntoIter};

use ahash::{AHashMap, AHashSet};
use compiler::grammar::{
    actions::action_redirect::{ByTime, Notify, Ret},
    instruction::Instruction,
    Capability,
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

#[derive(Clone)]
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
    pub(crate) no_capability_check: bool,

    // Functions
    pub(crate) functions: AHashMap<String, (u32, u32)>,
}

pub type Function = for<'x> fn(&'x Context<'x>, Vec<Variable>) -> Variable;

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
    pub(crate) environment: AHashMap<Cow<'static, str>, Variable>,
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
    pub(crate) envelope: Vec<(Envelope, Variable)>,
    pub(crate) metadata: Vec<(Metadata<String>, Cow<'x, str>)>,

    pub(crate) part: usize,
    pub(crate) part_iter: IntoIter<usize>,
    pub(crate) part_iter_stack: Vec<(usize, IntoIter<usize>)>,

    pub(crate) spam_status: SpamStatus,
    pub(crate) virus_status: VirusStatus,

    pub(crate) pos: usize,
    pub(crate) test_result: bool,
    pub(crate) script_cache: AHashMap<Script, Arc<Sieve>>,
    pub(crate) script_stack: Vec<ScriptStack>,
    pub(crate) vars_global: AHashMap<Cow<'static, str>, Variable>,
    pub(crate) vars_env: AHashMap<Cow<'static, str>, Variable>,
    pub(crate) vars_local: Vec<Variable>,
    pub(crate) vars_match: Vec<Variable>,
    pub(crate) expr_stack: Vec<Variable>,
    pub(crate) expr_pos: usize,

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
    SetEnvelope {
        envelope: Envelope,
        value: String,
    },
    Function {
        id: ExternalId,
        arguments: Vec<Variable>,
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
    FncResult(Variable),
    Script { name: Script, script: Arc<Sieve> },
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
