/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
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

pub mod compiler;
pub mod runtime;

pub(crate) const MAX_MATCH_VARIABLES: u32 = 63;
pub(crate) const MAX_LOCAL_VARIABLES: u32 = 256;

#[derive(Debug, Clone, Eq, PartialEq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub struct Sieve {
    instructions: Vec<Instruction>,
    num_vars: u32,
    num_match_vars: u32,
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
    #[cfg(test)]
    pub(crate) runtime: Runtime,
    #[cfg(not(test))]
    pub(crate) runtime: &'x Runtime,
    pub(crate) user_address: Cow<'x, str>,
    pub(crate) user_full_name: Cow<'x, str>,
    pub(crate) current_time: i64,

    pub(crate) message: Message<'x>,
    pub(crate) message_size: usize,
    pub(crate) envelope: Vec<(Envelope, Variable)>,
    pub(crate) metadata: Vec<(Metadata<String>, Cow<'x, str>)>,

    pub(crate) part: u32,
    pub(crate) part_iter: IntoIter<u32>,
    pub(crate) part_iter_stack: Vec<(u32, IntoIter<u32>)>,

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

#[derive(Debug, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
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

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
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
        compiler::grammar::Capability,
        runtime::{actions::action_mime::reset_test_boundary, Variable},
        Compiler, Context, Envelope, Event, FunctionMap, Input, Mailbox, Recipient, Runtime,
        SpamStatus, VirusStatus,
    };

    impl Variable {
        pub fn unwrap_string(self) -> String {
            self.to_string().into_owned()
        }
    }

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
            .with_function("trim", |_, v| match v.into_iter().next().unwrap() {
                crate::runtime::Variable::String(s) => s.trim().to_string().into(),
                v => v.to_string().into(),
            })
            .with_function("len", |_, v| v[0].to_string().len().into())
            .with_function("count", |_, v| {
                v[0].as_array().map_or(0, |arr| arr.len()).into()
            })
            .with_function("to_lowercase", |_, v| {
                v[0].to_string().to_lowercase().to_string().into()
            })
            .with_function("to_uppercase", |_, v| {
                v[0].to_string().to_uppercase().to_string().into()
            })
            .with_function("is_uppercase", |_, v| {
                v[0].to_string()
                    .as_ref()
                    .chars()
                    .filter(|c| c.is_alphabetic())
                    .all(|c| c.is_uppercase())
                    .into()
            })
            .with_function("is_ascii", |_, v| {
                v[0].to_string().as_ref().is_ascii().into()
            })
            .with_function("char_count", |_, v| {
                v[0].to_string().as_ref().chars().count().into()
            })
            .with_function("lines", |_, v| {
                v[0].to_string()
                    .lines()
                    .map(|line| Variable::from(line.to_string()))
                    .collect::<Vec<_>>()
                    .into()
            })
            .with_function_args(
                "contains",
                |_, v| v[0].to_string().contains(v[1].to_string().as_ref()).into(),
                2,
            )
            .with_function_args(
                "eq_lowercase",
                |_, v| {
                    v[0].to_string()
                        .as_ref()
                        .eq_ignore_ascii_case(v[1].to_string().as_ref())
                        .into()
                },
                2,
            )
            .with_function_args(
                "concat_three",
                |_, v| format!("{}-{}-{}", v[0], v[1], v[2]).into(),
                3,
            )
            .with_function_args(
                "in_array",
                |_, v| {
                    v[0].as_array()
                        .is_some_and(|arr| arr.contains(&v[1]))
                        .into()
                },
                2,
            )
            .with_external_function("ext_zero", 0, 0)
            .with_external_function("ext_one", 1, 1)
            .with_external_function("ext_two", 2, 2)
            .with_external_function("ext_three", 3, 3)
            .with_external_function("ext_true", 4, 0)
            .with_external_function("ext_false", 5, 0);
        let mut compiler = Compiler::new()
            .with_max_string_size(10240)
            .register_functions(&mut fnc_map);

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
                .with_capability(Capability::While)
                .with_capability(Capability::Expressions)
                .with_functions(&mut fnc_map.clone());
            let mut instance = Context::new(
                &runtime,
                Message {
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
                    ..Default::default()
                },
            );
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
                    Event::Function { id, arguments } => {
                        if id == u32::MAX {
                            // Test functions
                            input = Input::True;
                            let mut arguments = arguments.into_iter();
                            let command = arguments.next().unwrap().unwrap_string();
                            let mut params =
                                arguments.map(|arg| arg.unwrap_string()).collect::<Vec<_>>();

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
                                                .or_default()
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
                            let result = match id {
                                0 => Variable::from("my_value"),
                                1 => Variable::from(arguments[0].to_string().to_uppercase()),
                                2 => Variable::from(format!(
                                    "{}-{}",
                                    arguments[0].to_string(),
                                    arguments[1].to_string()
                                )),
                                3 => Variable::from(format!(
                                    "{}-{}-{}",
                                    arguments[0].to_string(),
                                    arguments[1].to_string(),
                                    arguments[2].to_string()
                                )),
                                4 => true.into(),
                                5 => false.into(),
                                _ => {
                                    panic!("Unknown external function {id}");
                                }
                            };

                            input = result.into();
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
