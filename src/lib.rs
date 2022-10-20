/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
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

use std::{borrow::Cow, sync::Arc, vec::IntoIter};

use ahash::{AHashMap, AHashSet};
use compiler::grammar::{
    actions::action_redirect::{ByTime, Notify, Ret},
    instruction::Instruction,
    Capability,
};
use mail_parser::{HeaderName, Message};
use runtime::context::ScriptStack;
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
    pub(crate) max_variable_size: usize,
    pub(crate) max_nested_blocks: usize,
    pub(crate) max_nested_tests: usize,
    pub(crate) max_nested_foreverypart: usize,
    pub(crate) max_match_variables: usize,
    pub(crate) max_local_variables: usize,
    pub(crate) max_header_size: usize,
    pub(crate) max_includes: usize,
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub(crate) allowed_capabilities: AHashSet<Capability>,
    pub(crate) valid_notification_uris: AHashSet<Cow<'static, str>>,
    pub(crate) valid_ext_lists: AHashSet<Cow<'static, str>>,
    pub(crate) protected_headers: Vec<HeaderName<'static>>,
    pub(crate) environment: AHashMap<String, Cow<'static, str>>,
    pub(crate) metadata: Vec<(Metadata<String>, Cow<'static, str>)>,
    pub(crate) include_scripts: AHashMap<String, Arc<Sieve>>,

    pub(crate) max_include_scripts: usize,
    pub(crate) cpu_limit: usize,
    pub(crate) max_memory: usize,
    pub(crate) max_variable_size: usize,
    pub(crate) max_redirects: usize,
    pub(crate) max_received_headers: usize,
    pub(crate) max_header_size: usize,

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
    pub(crate) envelope: Vec<(Envelope, Cow<'x, str>)>,
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
    pub(crate) vars_global: AHashMap<String, String>,
    pub(crate) vars_env: AHashMap<String, Cow<'x, str>>,
    pub(crate) vars_local: Vec<String>,
    pub(crate) vars_match: Vec<String>,

    pub(crate) actions: Vec<Action>,
    pub(crate) messages: Vec<Cow<'x, [u8]>>,
    pub(crate) last_message_id: usize,

    pub(crate) has_changes: bool,
    pub(crate) num_redirects: usize,
    pub(crate) num_instructions: usize,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Script {
    Personal(String),
    Global(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Action {
    Keep {
        flags: Vec<String>,
        message_id: usize,
    },
    Discard,
    Reject {
        reason: String,
    },
    Ereject {
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
        expiry: Expiry,
    },
    Execute {
        command: String,
        arguments: Vec<String>,
    },

    #[cfg(test)]
    TestCommand {
        command: String,
        params: Vec<String>,
    },
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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
pub enum Expiry {
    Seconds(u64),
    LastSeconds(u64),
    None,
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
        parsers::MessageStream, Addr, Encoding, HeaderValue, Message, MessagePart, PartType,
    };

    use crate::{
        compiler::grammar::Capability, runtime::actions::action_mime::reset_test_boundary, Action,
        Compiler, Envelope, Event, Input, Mailbox, Recipient, Runtime, SpamStatus, VirusStatus,
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
                .contains("error")
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
        let mut compiler = Compiler::new();
        let mut ancestors = script_path.ancestors();
        ancestors.next();
        let base_path = ancestors.next().unwrap();
        let script = compiler
            .compile(&add_crlf(&fs::read(&script_path).unwrap()))
            .unwrap();

        let mut input = Input::script("", script);
        let mut current_test = String::new();
        let mut raw_message_: Option<Vec<u8>> = None;
        let mut prev_state = None;
        let mut mailboxes = Vec::new();
        let mut lists: AHashMap<String, AHashSet<String>> = AHashMap::new();
        let mut duplicated_ids = AHashSet::new();

        'outer: loop {
            let runtime = Runtime::new()
                .with_protected_header("Auto-Submitted")
                .with_protected_header("Received")
                .with_valid_notification_uri("mailto")
                .with_capability(Capability::Execute);
            let mut instance = runtime.filter(b"");
            let raw_message = raw_message_.take().unwrap_or_default();
            instance.message = Message::parse(&raw_message).unwrap_or_else(|| Message {
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
            if let HeaderValue::Address(Addr {
                address: Some(addr),
                ..
            }) = instance.message.get_from()
            {
                instance.set_envelope(Envelope::From, addr.to_string());
            }
            if let HeaderValue::Address(Addr {
                address: Some(addr),
                ..
            }) = instance.message.get_to()
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
                        include_path.push(format!("{}.sieve", name));

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
                        for action in &instance.actions {
                            if let Action::FileInto { folder, create, .. } = action {
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
                    Event::Execute { command, arguments } => {
                        assert_eq!(arguments, ["param1", "param2"]);
                        input = (if command.eq_ignore_ascii_case("always_succeed") {
                            true
                        } else if command.eq_ignore_ascii_case("always_fail") {
                            false
                        } else {
                            panic!("Unknown command {}", command);
                        })
                        .into();
                    }

                    Event::TestCommand {
                        command,
                        mut params,
                    } => {
                        input = Input::True;

                        match command.as_str() {
                            "test" => {
                                current_test = params.pop().unwrap();
                                println!("Running test '{}'...", current_test);
                            }
                            "test_set" => {
                                let mut params = params.into_iter();
                                let target = params.next().expect("test_set parameter");
                                if target == "message" {
                                    let value = params.next().unwrap();
                                    raw_message_ = if value.eq_ignore_ascii_case(":smtp") {
                                        let mut message = None;
                                        for action in instance.actions.iter().rev() {
                                            if let Action::SendMessage { message_id, .. } = action {
                                                let message_ = &instance.messages[*message_id];
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
                                } else if let Some(envelope) = target.strip_prefix("envelope.") {
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
                                    panic!("test_set {} not implemented.", target);
                                }
                            }
                            "test_message" => {
                                let mut params = params.into_iter();
                                input = match params.next().unwrap().as_str() {
                                    ":folder" => {
                                        let folder_name = params.next().expect("test_message folder name");
                                        instance.actions.iter().any(|a| if !folder_name.eq_ignore_ascii_case("INBOX") { 
                                                matches!(a, Action::FileInto { folder, .. } if folder == &folder_name )
                                            } else {
                                                matches!(a, Action::Keep { .. })
                                            })
                                    }
                                    ":smtp" => {
                                        instance.actions.iter().any(|a| matches!(a, Action::SendMessage { .. } ))
                                    }
                                    param => panic!("Invalid test_message param '{}'", param),
                                }.into();
                            }
                            "test_assert_message" => {
                                let expected_message = params.first().expect("test_set parameter");
                                let built_message = instance.build_message();
                                if expected_message.as_bytes() != built_message {
                                    //fs::write("_deleteme.json", serde_json::to_string_pretty(&Message::parse(&built_message).unwrap()).unwrap()).unwrap();
                                    print!("<[");
                                    print!("{}", String::from_utf8(built_message).unwrap());
                                    println!("]>");
                                    panic!("Message built incorrectly at '{}'", current_test);
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
                                                instance
                                                    .runtime
                                                    .set_protected_header(header_name.to_string());
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
                                    param => panic!("Invalid test_config_set param '{}'", param),
                                }
                            }
                            "test_result_execute" => {
                                input = (instance.actions.iter().any(|a| {
                                    matches!(
                                        a,
                                        Action::Keep { .. }
                                            | Action::FileInto { .. }
                                            | Action::SendMessage { .. }
                                    )
                                }))
                                .into();
                            }
                            "test_result_action" => {
                                let param = params.first().expect("test_result_action parameter");
                                input = if param == "reject" {
                                    (instance
                                        .actions
                                        .iter()
                                        .any(|a| matches!(a, Action::Reject { .. })))
                                    .into()
                                } else if param == "redirect" {
                                    let param =
                                        params.last().expect("test_result_action redirect address");
                                    (instance
                                        .actions
                                        .iter()
                                        .any(|a| matches!(a, Action::SendMessage { recipient: Recipient::Address(address), .. } if address == param)))
                                    .into()
                                } else if param == "keep" {
                                    (instance
                                        .actions
                                        .iter()
                                        .any(|a| matches!(a, Action::Keep { .. })))
                                    .into()
                                } else if param == "send_message" {
                                    (instance
                                        .actions
                                        .iter()
                                        .any(|a| matches!(a, Action::SendMessage { .. })))
                                    .into()
                                } else {
                                    panic!("test_result_action {} not implemented", param);
                                };
                            }
                            "test_result_action_count" => {
                                input = (instance.actions.len()
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
                                instance.actions = vec![Action::Keep {
                                    flags: vec![],
                                    message_id: 0,
                                }];
                                instance.metadata.clear();
                                instance.messages.clear();
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
                                panic!("Test '{}' failed: {}", current_test, params.pop().unwrap());
                            }
                            _ => panic!("Test command {} not implemented.", command),
                        }
                    }
                }
            }

            return;
        }

        //let mut files = Vec::new();
        //let mut items = BTreeSet::new();

        //read_dir(PathBuf::from("tests"), &mut files);
        //for file in files {
        /*println!("parsing {:?}", file);
        let bytes = fs::read(&file).unwrap();
        let tokens = tokenize(&bytes).unwrap();
        for token in tokens {
            if let Token::Identifier(id) = token.token {
                //items.insert(id.to_lowercase());
            }
        }*/

        /*if file.as_os_str().to_str().unwrap().contains("lexer.svtest") {
            println!("{:#?}", tokens);
            break;
        }*/
        //}

        /*fs::write(
            "identifiers.txt",
            items.into_iter().collect::<Vec<_>>().join("\n"),
        )
        .unwrap();*/
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
