/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    Arena, Compiler, Context, Envelope, FunctionMap, Runtime, Sieve, SpamStatus, VirusStatus,
    compiler::grammar::Capability,
    runtime::{
        Variable,
        actions::action_mime::reset_test_boundary,
        handler::{Action, Handler, Input, Mailbox, Recipient, Reply, Script, Status},
    },
};
use ahash::{AHashMap, AHashSet};
use mail_parser::{
    Encoding, HeaderValue, Message, MessageParser, MessagePart, PartType, parsers::MessageStream,
};
use std::{
    fs,
    path::{Path, PathBuf},
};

mod regressions;

impl Variable<'_> {
    pub fn unwrap_string(self) -> String {
        self.to_string().into_owned()
    }
}

#[derive(Debug, Clone, PartialEq)]
enum Recorded {
    Keep {
        flags: Vec<String>,
    },
    FileInto {
        folder: String,
        create: bool,
    },
    SendMessage {
        address: Option<String>,
        message_id: usize,
    },
    CreatedMessage {
        message_id: usize,
        message: Vec<u8>,
    },
    Reject,
    Discard,
    Notify,
    SetEnvelope,
}

struct TestHandler {
    base_path: PathBuf,
    compiler: Compiler,
    pending: Option<(String, Vec<String>)>,
    mailboxes: Vec<String>,
    lists: AHashMap<String, AHashSet<String>>,
    duplicated_ids: AHashSet<String>,
    actions: Vec<Recorded>,
}

impl<'x> Handler<'x> for TestHandler {
    fn include_script(
        &mut self,
        _: &Context<'x>,
        name: Script<'_>,
        optional: bool,
    ) -> Reply<Option<&'x Sieve<'x>>> {
        let mut include_path = self.base_path.clone();
        include_path.push(if matches!(name, Script::Personal(_)) {
            "included"
        } else {
            "included-global"
        });
        include_path.push(format!("{}.sieve", name.name()));

        if let Ok(bytes) = fs::read(include_path.as_path()) {
            let script = self.compiler.compile(&add_crlf(&bytes)).unwrap();
            Reply::Ready(Some(Box::leak(Box::new(script))))
        } else if optional {
            Reply::Ready(None)
        } else {
            panic!("Script {} not found.", include_path.display());
        }
    }

    fn mailbox_exists(
        &mut self,
        _: &Context<'x>,
        mailboxes: &[Mailbox<'_>],
        special_use: &[&str],
    ) -> Reply<bool> {
        for action in &self.actions {
            if let Recorded::FileInto { folder, create } = action
                && *create
                && !self.mailboxes.contains(folder)
            {
                self.mailboxes.push(folder.to_string());
            }
        }
        Reply::Ready(
            special_use.is_empty()
                && mailboxes.iter().all(|n| {
                    if let Mailbox::Name(n) = n {
                        self.mailboxes.iter().any(|m| m == n)
                    } else {
                        false
                    }
                }),
        )
    }

    fn list_contains(
        &mut self,
        _: &Context<'x>,
        lists: &[&str],
        values: &[&str],
        _: crate::MatchAs,
    ) -> Reply<bool> {
        let mut result = false;
        'list: for list in lists {
            if let Some(list) = self.lists.get(*list) {
                for value in values {
                    if list.contains(*value) {
                        result = true;
                        break 'list;
                    }
                }
            }
        }
        Reply::Ready(result)
    }

    fn duplicate_id(&mut self, _: &Context<'x>, id: &str, _: u64, _: bool) -> Reply<bool> {
        Reply::Ready(self.duplicated_ids.contains(id))
    }

    fn function(
        &mut self,
        _: &Context<'x>,
        id: u32,
        arguments: &[Variable<'_>],
    ) -> Reply<Variable<'x>> {
        if id == u32::MAX {
            let mut arguments = arguments.iter().map(|arg| arg.to_string().into_owned());
            let command = arguments.next().unwrap();
            self.pending = Some((command, arguments.collect()));
            Reply::Pending
        } else {
            Reply::Ready(match id {
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
            })
        }
    }

    fn action(&mut self, _: &Context<'x>, action: Action<'_>) -> Reply<()> {
        self.actions.push(match action {
            Action::Keep { flags, .. } => Recorded::Keep {
                flags: flags.iter().map(|f| f.to_string()).collect(),
            },
            Action::Discard => Recorded::Discard,
            Action::Reject { .. } => Recorded::Reject,
            Action::FileInto { folder, create, .. } => Recorded::FileInto {
                folder: folder.to_string(),
                create,
            },
            Action::SendMessage {
                recipient,
                message_id,
                ..
            } => Recorded::SendMessage {
                address: match recipient {
                    Recipient::Address(address) => Some(address.to_string()),
                    _ => None,
                },
                message_id,
            },
            Action::Notify { .. } => Recorded::Notify,
            Action::SetEnvelope { .. } => Recorded::SetEnvelope,
            Action::CreatedMessage {
                message_id,
                message,
            } => Recorded::CreatedMessage {
                message_id,
                message,
            },
        });
        Reply::Ready(())
    }
}

#[test]
fn test_suite() {
    let mut tests = Vec::new();
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests");

    read_dir(path, &mut tests);
    let filter = std::env::var("SVTEST_FILTER").unwrap_or_default();

    let mut failures = Vec::new();
    for test in tests {
        if !filter.is_empty() && !test.display().to_string().contains(&filter) {
            continue;
        }
        println!("===== {} =====", test.display());
        if std::panic::catch_unwind(|| run_test(&test)).is_err() {
            failures.push(test.display().to_string());
        }
    }
    assert!(
        failures.is_empty(),
        "{} test files failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
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

fn empty_message() -> Message<'static> {
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
    }
}

fn parse_message(raw_message: &'static [u8]) -> Message<'static> {
    MessageParser::new()
        .parse(raw_message)
        .unwrap_or_else(empty_message)
}

fn run_test(script_path: &Path) {
    let mut fnc_map = FunctionMap::new()
        .with_function("trim", |_, v| match &v[0] {
            Variable::String(s) => s.trim().to_string().into(),
            v => v.to_string().into_owned().into(),
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
    let base_path = ancestors.next().unwrap().to_path_buf();
    let script: &'static Sieve<'static> = Box::leak(Box::new(
        compiler
            .compile(&add_crlf(&fs::read(script_path).unwrap()))
            .unwrap(),
    ));

    let mut runtime = Runtime::new()
        .with_protected_header("Auto-Submitted")
        .with_protected_header("Received")
        .with_valid_notification_uri("mailto")
        .with_max_out_messages(100)
        .with_capability(Capability::While)
        .with_capability(Capability::Expressions)
        .with_functions(&mut fnc_map.clone());
    let mut runtime_ref: &'static Runtime = Box::leak(Box::new(runtime.clone()));
    let arena: &'static mut Arena = Box::leak(Box::new(Arena::new()));
    let mut instance = Context::new(runtime_ref, empty_message(), script, arena);
    instance.set_env_variable("vnd.stalwart.default_mailbox", "INBOX");
    instance.set_env_variable("vnd.stalwart.username", "john.doe");
    instance.set_user_address("MAILER-DAEMON");

    let mut handler = TestHandler {
        base_path,
        compiler: compiler.clone(),
        pending: None,
        mailboxes: Vec::new(),
        lists: AHashMap::new(),
        duplicated_ids: AHashSet::new(),
        actions: Vec::new(),
    };
    let mut current_test = String::new();

    loop {
        let status = instance.run(&mut handler).unwrap();
        if status == Status::Finished {
            break;
        }
        let Some((command, mut params)) = handler.pending.take() else {
            panic!("Suspended without a pending test command");
        };
        let mut input = Input::Bool(true);

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
                    let raw_message: Vec<u8> = if value.eq_ignore_ascii_case(":smtp") {
                        let mut message = None;
                        for action in handler.actions.iter().rev() {
                            if let Recorded::SendMessage { message_id, .. } = action {
                                let message_ = handler
                                    .actions
                                    .iter()
                                    .find_map(|item| {
                                        if let Recorded::CreatedMessage {
                                            message_id: message_id_,
                                            message,
                                        } = item
                                            && message_id == message_id_
                                        {
                                            return Some(message);
                                        }
                                        None
                                    })
                                    .unwrap();
                                message = message_.into();
                                break;
                            }
                        }
                        message.expect("No SMTP message found").to_vec()
                    } else {
                        value.into_bytes()
                    };
                    let raw_message: &'static [u8] = Box::leak(raw_message.into_boxed_slice());
                    let message = parse_message(raw_message);
                    instance.set_message(message, raw_message.len());
                    instance.clear_envelope();
                    if let Some(addr) = instance
                        .message
                        .from()
                        .and_then(|a| a.first())
                        .and_then(|a| a.address.as_ref())
                    {
                        let addr = addr.to_string();
                        instance.set_envelope(Envelope::From, addr);
                    }
                    if let Some(addr) = instance
                        .message
                        .to()
                        .and_then(|a| a.first())
                        .and_then(|a| a.address.as_ref())
                    {
                        let addr = addr.to_string();
                        instance.set_envelope(Envelope::To, addr);
                    }
                } else if let Some(envelope) = target.strip_prefix("envelope.") {
                    let envelope = Envelope::try_from(envelope.to_string()).unwrap();
                    instance.envelope.retain(|(e, _)| e != &envelope);
                    instance.set_envelope(envelope, params.next().unwrap());
                } else if target == "currentdate" {
                    let bytes = params.next().unwrap().into_bytes();
                    if let HeaderValue::DateTime(dt) = MessageStream::new(&bytes).parse_date() {
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
                        matches!(&instance.final_action, Some(Action::Keep { .. }))
                            || handler.actions.iter().any(|a| {
                                if !folder_name.eq_ignore_ascii_case("INBOX") {
                                    matches!(a, Recorded::FileInto { folder, .. } if folder == &folder_name)
                                } else {
                                    matches!(a, Recorded::Keep { .. })
                                }
                            })
                    }
                    ":smtp" => handler
                        .actions
                        .iter()
                        .any(|a| matches!(a, Recorded::SendMessage { .. })),
                    param => panic!("Invalid test_message param '{param}'"),
                }
                .into();
            }
            "test_assert_message" => {
                let expected_message = params.first().expect("test_set parameter");
                let built_message = instance.build_message();
                if expected_message.as_bytes() != built_message {
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
                let mut runtime_changed = true;

                match name.as_str() {
                    "sieve_editheader_protected"
                    | "sieve_editheader_forbid_add"
                    | "sieve_editheader_forbid_delete" => {
                        if !value.is_empty() {
                            for header_name in value.split(' ') {
                                runtime.set_protected_header(header_name.to_string());
                            }
                        } else {
                            runtime.protected_headers.clear();
                        }
                    }
                    "sieve_variables_max_variable_size" => {
                        runtime.set_max_variable_size(value.parse().unwrap());
                    }
                    "sieve_valid_ext_list" => {
                        runtime.set_valid_ext_list(value);
                    }
                    "sieve_ext_list_item" => {
                        handler
                            .lists
                            .entry(value)
                            .or_default()
                            .insert(params.next().expect("list item value"));
                        runtime_changed = false;
                    }
                    "sieve_duplicated_id" => {
                        handler.duplicated_ids.insert(value);
                        runtime_changed = false;
                    }
                    "sieve_user_email" => {
                        instance.set_user_address(value);
                        runtime_changed = false;
                    }
                    "sieve_vacation_use_original_recipient" => {
                        runtime.set_vacation_use_orig_rcpt(value.eq_ignore_ascii_case("yes"));
                    }
                    "sieve_vacation_default_subject" => {
                        runtime.set_vacation_default_subject(value);
                    }
                    "sieve_vacation_default_subject_template" => {
                        runtime.set_vacation_subject_prefix(value);
                    }
                    "sieve_spam_status" => {
                        instance.set_spam_status(SpamStatus::from_number(value.parse().unwrap()));
                        runtime_changed = false;
                    }
                    "sieve_spam_status_plus" => {
                        instance.set_spam_status(match value.parse::<u32>().unwrap() {
                            0 => SpamStatus::Unknown,
                            100.. => SpamStatus::Spam,
                            n => SpamStatus::MaybeSpam((n as f64) / 100.0),
                        });
                        runtime_changed = false;
                    }
                    "sieve_virus_status" => {
                        instance.set_virus_status(VirusStatus::from_number(value.parse().unwrap()));
                        runtime_changed = false;
                    }
                    "sieve_editheader_max_header_size" => {
                        let mhs = if !value.is_empty() {
                            value.parse::<usize>().unwrap()
                        } else {
                            1024
                        };
                        runtime.set_max_header_size(mhs);
                        compiler.set_max_header_size(mhs);
                        handler.compiler = compiler.clone();
                    }
                    "sieve_include_max_includes" => {
                        compiler.set_max_includes(if !value.is_empty() {
                            value.parse::<usize>().unwrap()
                        } else {
                            3
                        });
                        handler.compiler = compiler.clone();
                        runtime_changed = false;
                    }
                    "sieve_include_max_nesting_depth" => {
                        compiler.set_max_nested_blocks(if !value.is_empty() {
                            value.parse::<usize>().unwrap()
                        } else {
                            3
                        });
                        handler.compiler = compiler.clone();
                        runtime_changed = false;
                    }
                    param => panic!("Invalid test_config_set param '{param}'"),
                }

                if runtime_changed {
                    runtime_ref = Box::leak(Box::new(runtime.clone()));
                    instance = instance.with_runtime(runtime_ref);
                }
            }
            "test_result_execute" => {
                input = (matches!(&instance.final_action, Some(Action::Keep { .. }))
                    || handler.actions.iter().any(|a| {
                        matches!(
                            a,
                            Recorded::Keep { .. }
                                | Recorded::FileInto { .. }
                                | Recorded::SendMessage { .. }
                        )
                    }))
                .into();
            }
            "test_result_action" => {
                let param = params.first().expect("test_result_action parameter");
                input = if param == "reject" {
                    handler
                        .actions
                        .iter()
                        .any(|a| matches!(a, Recorded::Reject))
                        .into()
                } else if param == "redirect" {
                    let param = params.last().expect("test_result_action redirect address");
                    handler
                        .actions
                        .iter()
                        .any(|a| matches!(a, Recorded::SendMessage { address: Some(address), .. } if address == param))
                        .into()
                } else if param == "keep" {
                    (matches!(&instance.final_action, Some(Action::Keep { .. }))
                        || handler
                            .actions
                            .iter()
                            .any(|a| matches!(a, Recorded::Keep { .. })))
                    .into()
                } else if param == "send_message" {
                    handler
                        .actions
                        .iter()
                        .any(|a| matches!(a, Recorded::SendMessage { .. }))
                        .into()
                } else {
                    panic!("test_result_action {param} not implemented");
                };
            }
            "test_result_action_count" => {
                input = (handler.actions.len()
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
                handler
                    .mailboxes
                    .push(params.pop().expect("mailbox to create"));
            }
            "test_result_reset" => {
                handler.actions.clear();
                instance.final_action = Some(Action::Keep {
                    flags: &[],
                    message_id: 0,
                });
                instance.metadata.clear();
                instance.has_changes = false;
                instance.num_redirects = 0;
                if runtime.vacation_use_orig_rcpt {
                    runtime.vacation_use_orig_rcpt = false;
                    runtime_ref = Box::leak(Box::new(runtime.clone()));
                    instance = instance.with_runtime(runtime_ref);
                }
                handler.mailboxes.clear();
                handler.lists.clear();
                reset_test_boundary();
            }
            "test_script_compile" => {
                let mut include_path = handler.base_path.clone();
                include_path.push(params.first().unwrap());

                if let Ok(bytes) = fs::read(include_path.as_path()) {
                    let result = compiler.compile(&add_crlf(&bytes));
                    input = result.is_ok().into();
                } else {
                    panic!("Script {} not found.", include_path.display());
                }
            }
            "test_config_reload" => (),
            "test_fail" => {
                panic!("Test '{}' failed: {}", current_test, params.pop().unwrap());
            }
            _ => panic!("Test command {command} not implemented."),
        }

        instance.resume(input);
    }
}

const ROUND_TRIP_SCRIPT: &str = concat!(
    "require [\"variables\", \"fileinto\"];\n",
    "if header :contains \"subject\" \"hello\" { set \"greeting\" \"hello\"; }\n",
    "if header :matches \"subject\" \"*world*\" { fileinto \"hello\"; }\n"
);

#[test]
fn bytecode_round_trip() {
    let script = Compiler::new()
        .compile(&add_crlf(ROUND_TRIP_SCRIPT.as_bytes()))
        .unwrap();

    let bytes = script.to_bytes();
    let restored = Sieve::from_bytes(&bytes).unwrap();
    assert_eq!(script, restored);
    assert_eq!(restored.to_bytes(), bytes);
    assert_eq!(script, restored.into_owned());
    assert_eq!(
        script,
        unsafe { Sieve::from_bytes_unchecked(&bytes) }.unwrap()
    );

    let mut stale = bytes.clone();
    stale[4] = 0xff;
    assert!(matches!(
        Sieve::from_bytes(&stale),
        Err(crate::LoadError::UnsupportedVersion(_))
    ));
    assert!(matches!(
        Sieve::from_bytes(&[]),
        Err(crate::LoadError::Truncated)
    ));
    assert!(matches!(
        Sieve::from_bytes(&bytes[..bytes.len() - 1]),
        Err(crate::LoadError::Truncated)
    ));
    for at in 40..bytes.len() {
        let mut corrupted = bytes.clone();
        corrupted[at] ^= 0x5a;
        let _ = Sieve::from_bytes(&corrupted);
    }
}

pub(crate) fn add_crlf(bytes: &[u8]) -> Vec<u8> {
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

struct NullHandler;

impl<'x> Handler<'x> for NullHandler {}

fn patch_record(bytes: &mut [u8], tag: u8, d: u32) -> bool {
    let code_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    let records_len = u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]) as usize;
    let start = 40 + code_len;
    let mut patched = false;
    for rec in bytes[start..start + records_len].as_chunks_mut::<16>().0 {
        if rec[0] == tag {
            rec[4..8].copy_from_slice(&d.to_le_bytes());
            patched = true;
        }
    }
    patched
}

fn run_crafted(bytes: &[u8]) -> Result<Status, crate::runtime::RuntimeError> {
    let runtime = Runtime::new()
        .with_cpu_limit(1000)
        .with_capability(Capability::Expressions);
    let sieve = unsafe { Sieve::from_bytes_unchecked(bytes) }.unwrap();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &sieve, &mut arena);
    ctx.run(&mut NullHandler)
}

#[test]
fn crafted_expression_jump_is_rejected() {
    let script = Compiler::new()
        .compile(
            b"require [\"variables\", \"vnd.stalwart.expressions\"];\r\nlet \"x\" \"1 || 2\";\r\n",
        )
        .unwrap();
    let mut bytes = script.to_bytes();
    assert!(patch_record(
        &mut bytes,
        crate::bytecode::rec::tag::JMP_IF,
        u32::MAX
    ));
    assert!(matches!(
        Sieve::from_bytes(&bytes),
        Err(crate::LoadError::Corrupted)
    ));
    assert!(matches!(
        run_crafted(&bytes),
        Err(crate::runtime::RuntimeError::InvalidBytecode)
    ));
}

#[test]
fn crafted_function_id_is_rejected() {
    let mut fnc_map = FunctionMap::new().with_external_function("ext", 1, 0);
    let script = Compiler::new()
        .register_functions(&mut fnc_map)
        .compile(
            b"require [\"variables\", \"vnd.stalwart.expressions\"];\r\nlet \"x\" \"ext()\";\r\n",
        )
        .unwrap();
    let mut bytes = script.to_bytes();
    assert!(patch_record(
        &mut bytes,
        crate::bytecode::rec::tag::CALL,
        u32::MAX
    ));
    assert!(matches!(
        Sieve::from_bytes(&bytes),
        Err(crate::LoadError::Corrupted)
    ));
    assert!(matches!(
        run_crafted(&bytes),
        Err(crate::runtime::RuntimeError::InvalidBytecode)
    ));
}
