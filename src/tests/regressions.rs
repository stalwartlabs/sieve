/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{NullHandler, add_crlf, empty_message};
use crate::{
    Arena, Compiler, Context, Envelope, FunctionMap, LoadError, Runtime, ScriptArena, Sieve,
    compiler::{
        ReceivedHostname, ReceivedPart,
        grammar::{Capability, actions::action_redirect::Notify},
    },
    runtime::{
        RuntimeError, Variable,
        handler::{Action, Handler, Input, MessageSource, Recipient, Reply, Script, Status},
    },
};
use mail_parser::{HeaderName, HeaderValue, MessageParser};

fn compile(script: &str) -> Sieve<'static> {
    Compiler::new()
        .compile(&add_crlf(script.as_bytes()))
        .unwrap()
}

fn runtime() -> Runtime {
    Runtime::new()
        .with_capability(Capability::While)
        .with_capability(Capability::Expressions)
}

#[derive(Default)]
struct RecordingHandler {
    keeps: usize,
    discards: usize,
    rejects: usize,
    sends: Vec<(MessageSource, bool)>,
    empty_recipients: usize,
    fileinto_flags: Vec<Vec<String>>,
    reject_fileinto: bool,
    park_includes: bool,
}

impl<'x> Handler<'x> for RecordingHandler {
    fn include_script(
        &mut self,
        _: &Context<'x>,
        _: Script<'_>,
        _: bool,
    ) -> Reply<Option<&'x Sieve<'x>>> {
        if self.park_includes {
            Reply::Pending
        } else {
            Reply::Ready(None)
        }
    }

    fn action(&mut self, _: &Context<'x>, action: Action<'x>) -> Reply<()> {
        match action {
            Action::Keep { .. } => self.keeps += 1,
            Action::Discard => self.discards += 1,
            Action::Reject { .. } => self.rejects += 1,
            Action::SendMessage {
                source,
                notify,
                recipient,
                ..
            } => {
                if matches!(recipient, Recipient::Address("")) {
                    self.empty_recipients += 1;
                }
                self.sends.push((source, matches!(notify, Notify::Never)));
            }
            Action::FileInto { .. } if self.reject_fileinto => {
                return Reply::Error(RuntimeError::CapabilityNotAllowed(Capability::FileInto));
            }
            Action::FileInto { flags, .. } => self
                .fileinto_flags
                .push(flags.iter().map(|flag| flag.to_string()).collect()),
            _ => (),
        }
        Reply::Ready(())
    }
}

fn fn_received_from<'x>(ctx: &Context<'x>, _: &[Variable<'x>]) -> Variable<'x> {
    ctx.message()
        .parts
        .first()
        .and_then(|part| {
            part.headers
                .iter()
                .find(|header| header.name == HeaderName::Received)
        })
        .and_then(|header| match &header.value {
            HeaderValue::Received(rcvd) => {
                ctx.received_part(&ReceivedPart::From(ReceivedHostname::Name), rcvd)
            }
            _ => None,
        })
        .unwrap_or_default()
}

struct IncludeHandler<'x> {
    arena: &'x ScriptArena,
    asked: Vec<String>,
}

impl<'x> Handler<'x> for IncludeHandler<'x> {
    fn include_script(
        &mut self,
        _: &Context<'x>,
        name: Script<'_>,
        _: bool,
    ) -> Reply<Option<&'x Sieve<'x>>> {
        self.asked.push(name.name().to_string());
        if name.name() == "inc" {
            Reply::Ready(Some(self.arena.push(compile(
                "require [\"variables\", \"include\"];\nset \"global.n\" \"${global.n}x\";\n",
            ))))
        } else {
            Reply::Ready(None)
        }
    }
}

fn exhausting_script(iterations: usize) -> Sieve<'static> {
    compile(&format!(
        "require [\"variables\", \"vnd.stalwart.expressions\", \"vnd.stalwart.while\"];\n\
         set \"a\" \"x\";\n\
         let \"i\" \"0\";\n\
         while \"i < {iterations}\" {{ set \"a\" \"${{a}}${{a}}\"; let \"i\" \"i + 1\"; }}\n"
    ))
}

#[test]
fn arena_exhaustion_returns_error() {
    let script = exhausting_script(4000);
    let runtime = runtime()
        .with_cpu_limit(1_000_000)
        .with_memory_limit(1 << 20);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    assert!(matches!(
        ctx.run(&mut NullHandler),
        Err(RuntimeError::MemoryLimitReached)
    ));
}

#[test]
fn arena_limit_applies_to_presized_arena() {
    let script = exhausting_script(4000);
    let runtime = runtime()
        .with_cpu_limit(1_000_000)
        .with_memory_limit(1 << 20);
    let mut arena = Arena::with_capacity(4 << 20);
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    assert_eq!(
        ctx.run(&mut NullHandler),
        Err(RuntimeError::MemoryLimitReached)
    );
}

#[test]
fn arena_limit_applies_after_reuse() {
    let mut arena = Arena::new();
    let generous = runtime()
        .with_cpu_limit(1_000_000)
        .with_memory_limit(64 << 20);
    let script = exhausting_script(600);
    {
        let mut ctx = Context::new(&generous, empty_message(), &script, &mut arena);
        assert!(matches!(ctx.run(&mut NullHandler), Ok(Status::Finished)));
    }
    assert!(arena.allocated_bytes() > 1 << 20);
    let strict = runtime()
        .with_cpu_limit(1_000_000)
        .with_memory_limit(1 << 20);
    let script = exhausting_script(4000);
    let mut ctx = Context::new(&strict, empty_message(), &script, &mut arena);
    assert_eq!(
        ctx.run(&mut NullHandler),
        Err(RuntimeError::MemoryLimitReached)
    );
}

#[test]
fn failed_allocation_never_reaches_handler() {
    let address = format!("{}@example.org", "a".repeat(900));
    let script = compile(&format!(
        "require [\"vnd.stalwart.expressions\", \"vnd.stalwart.while\"];\n\
         let \"i\" \"0\";\n\
         while \"i < 500\" {{ redirect \"{address}\"; let \"i\" \"i + 1\"; }}\n"
    ));
    let runtime = runtime()
        .with_cpu_limit(100_000)
        .with_memory_limit(32 << 10)
        .with_max_redirects(1000)
        .with_max_out_messages(1000);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert_eq!(ctx.run(&mut handler), Err(RuntimeError::MemoryLimitReached));
    assert!(!handler.sends.is_empty());
    assert_eq!(handler.empty_recipients, 0);
}

#[test]
fn dynamic_regexes_are_bounded_and_cached() {
    let script = compile(
        "require [\"variables\", \"regex\", \"vnd.stalwart.expressions\", \"vnd.stalwart.while\"];\n\
         set \"huge\" \"((a{100}){100}){100}\";\n\
         if string :regex \"aaa\" \"${huge}\" { discard; }\n\
         set \"ok\" \"^h.*o$\";\n\
         let \"i\" \"0\";\n\
         let \"n\" \"0\";\n\
         while \"i < 50\" {\n\
             if string :regex \"hello\" \"${ok}\" { let \"n\" \"n + 1\"; }\n\
             let \"i\" \"i + 1\";\n\
         }\n\
         if not eval \"n == 50\" { discard; }\n",
    );
    let runtime = runtime().with_cpu_limit(100_000);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.keeps, handler.discards), (1, 0));
}

#[test]
fn compiler_rejects_oversized_regex() {
    let result = Compiler::new().compile(
        b"require \"regex\";\r\nif header :regex \"subject\" \"((a{100}){100}){100}\" { keep; }\r\n",
    );
    assert!(result.is_err());
    assert!(
        Compiler::new()
            .compile(b"require \"regex\";\r\nif header :regex \"subject\" \"^h.*o$\" { keep; }\r\n")
            .is_ok()
    );
}

#[test]
fn owned_raw_message_is_copied_once() {
    let mut raw = b"Subject: Hello\r\n\r\n".to_vec();
    raw.resize(200 * 1024, b'a');
    let message = MessageParser::default().parse(&raw).unwrap().into_owned();
    let script = compile(
        "require [\"variables\", \"include\"];\n\
         set \"global.d\" \"${header.subject}\";\n\
         set \"global.d\" \"${header.subject}\";\n\
         set \"global.d\" \"${header.subject}\";\n\
         set \"global.d\" \"${header.subject}\";\n\
         set \"global.d\" \"${header.subject}\";\n\
         set \"global.d\" \"${header.subject}\";\n\
         if not string :is \"${global.d}\" \"Hello\" { discard; }\n",
    );
    let runtime = runtime().with_memory_limit(512 * 1024);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, message, &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(handler.keeps, 1);
    assert_eq!(handler.discards, 0);
    assert_eq!(
        ctx.global_variable("d").map(|v| v.to_string().into_owned()),
        Some("Hello".to_string())
    );
}

#[test]
fn resume_before_run_is_ignored() {
    let script = compile("discard;\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    ctx.resume(Input::Continue);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(handler.discards, 1);
    assert_eq!(handler.keeps, 0);
}

#[test]
fn include_once_records_only_included_scripts() {
    let script = compile(
        "require [\"include\", \"variables\"];\n\
         include :optional :once \"missing\";\n\
         include :optional :once \"missing\";\n\
         include :once \"inc\";\n\
         include :once \"inc\";\n\
         include :global :once \"sys\";\n\
         include :global :once \"sys\";\n",
    );
    let runtime = runtime().with_include_script(
        "sys",
        compile("require [\"variables\", \"include\"];\nset \"global.s\" \"${global.s}y\";\n"),
    );
    let scripts = ScriptArena::new();
    let mut handler = IncludeHandler {
        arena: &scripts,
        asked: Vec::new(),
    };
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(handler.asked, ["missing", "missing", "inc"]);
    assert_eq!(scripts.len(), 1);
    assert_eq!(
        ctx.global_variable("n").map(|v| v.to_string().into_owned()),
        Some("x".to_string())
    );
    assert_eq!(
        ctx.global_variable("s").map(|v| v.to_string().into_owned()),
        Some("y".to_string())
    );
}

#[test]
fn single_flag_variable_is_split_on_whitespace() {
    let raw = b"X-Flags: alpha beta\r\n\r\n";
    let message = MessageParser::default().parse(&raw[..]).unwrap();
    let script = compile(
        "require [\"imap4flags\", \"fileinto\", \"variables\"];\n\
         fileinto :flags \"${header.x-flags}\" \"one\";\n\
         fileinto :flags [\"${header.x-flags}\", \"gamma\"] \"two\";\n",
    );
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, message, &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(
        handler.fileinto_flags,
        [vec!["alpha", "beta"], vec!["alpha beta", "gamma"]]
    );
}

#[test]
fn runtime_filter_runs_a_script() {
    let script = compile("keep;\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = runtime.filter(b"Subject: x\r\n\r\nbody\r\n", &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(handler.keeps, 1);
}

fn section_lengths(bytes: &[u8]) -> [usize; 3] {
    let at = |off: usize| {
        u32::from_le_bytes([bytes[off], bytes[off + 1], bytes[off + 2], bytes[off + 3]]) as usize
    };
    [at(12), at(16), at(20)]
}

#[test]
fn crafted_regex_count_is_rejected() {
    let mut bytes =
        compile("require \"regex\";\nif header :regex \"subject\" \"a+\" { keep; }\n").to_bytes();
    assert!(Sieve::from_bytes(&bytes).is_ok());
    bytes[32..36].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Sieve::from_bytes(&bytes),
        Err(LoadError::Corrupted)
    ));
}

#[test]
fn crafted_header_count_is_rejected() {
    let mut bytes = compile("if header :is \"subject\" \"a\" { keep; }\n").to_bytes();
    let [code, records, blob] = section_lengths(&bytes);
    let names = 40 + code + records + blob;
    assert!(Sieve::from_bytes(&bytes).is_ok());
    bytes[names..names + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert!(matches!(
        Sieve::from_bytes(&bytes),
        Err(LoadError::Corrupted)
    ));
}

#[test]
fn crafted_variable_counts_are_rejected() {
    let mut bytes = compile(
        "require \"variables\";\nif header :matches \"subject\" \"*\" { set \"a\" \"${1}\"; }\n",
    )
    .to_bytes();
    assert!(Sieve::from_bytes(&bytes).is_ok());
    let mut no_locals = bytes.clone();
    no_locals[8..10].copy_from_slice(&0u16.to_le_bytes());
    assert!(matches!(
        Sieve::from_bytes(&no_locals),
        Err(LoadError::Corrupted)
    ));
    bytes[10..12].copy_from_slice(&0u16.to_le_bytes());
    assert!(matches!(
        Sieve::from_bytes(&bytes),
        Err(LoadError::Corrupted)
    ));
}

#[test]
fn load_errors_are_classified() {
    let mut bytes = compile("keep;\n").to_bytes();
    assert!(matches!(
        Sieve::from_bytes(&bytes[..20]),
        Err(LoadError::Truncated)
    ));
    let mut truncated = bytes.clone();
    truncated.truncate(bytes.len() - 1);
    assert!(matches!(
        Sieve::from_bytes(&truncated),
        Err(LoadError::Truncated)
    ));
    bytes[0] ^= 0xff;
    assert!(matches!(
        Sieve::from_bytes(&bytes),
        Err(LoadError::Corrupted)
    ));
}

#[test]
fn runtime_errors_have_messages() {
    assert_eq!(
        RuntimeError::TooManyIncludes.to_string(),
        "Too many nested includes"
    );
    assert!(!RuntimeError::AwaitingInput.to_string().is_empty());
}

#[test]
fn handler_error_aborts_script_and_keeps() {
    let script = compile("require \"fileinto\";\nfileinto \"Spam\";\ndiscard;\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler {
        reject_fileinto: true,
        ..Default::default()
    };
    assert_eq!(
        ctx.run(&mut handler),
        Err(RuntimeError::CapabilityNotAllowed(Capability::FileInto))
    );
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(handler.keeps, 1);
    assert_eq!(handler.discards, 0);
    assert!(handler.fileinto_flags.is_empty());
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(handler.keeps, 1);
}

#[test]
fn missing_include_is_an_error() {
    let script = compile("require \"include\";\ninclude \"missing\";\ndiscard;\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert_eq!(
        ctx.run(&mut handler),
        Err(RuntimeError::ScriptNotFound("missing".to_string()))
    );
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.keeps, handler.discards), (1, 0));

    let script = compile("require \"include\";\ninclude :optional \"missing\";\ndiscard;\n");
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.keeps, handler.discards), (0, 1));
}

#[test]
fn missing_include_after_pending_is_an_error() {
    let script = compile("require \"include\";\ninclude \"missing\";\ndiscard;\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler {
        park_includes: true,
        ..Default::default()
    };
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Pending)));
    ctx.resume(Input::Script(None));
    assert_eq!(
        ctx.run(&mut handler),
        Err(RuntimeError::ScriptNotFound("missing".to_string()))
    );
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.keeps, handler.discards), (1, 0));
}

#[test]
fn error_action_keeps_message() {
    let script = compile("require \"ihave\";\ndiscard;\nerror \"boom\";\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert_eq!(
        ctx.run(&mut handler),
        Err(RuntimeError::ScriptErrorMessage("boom".to_string()))
    );
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.keeps, handler.discards), (1, 0));
}

#[test]
fn error_after_reject_does_not_keep() {
    let script = compile("require [\"reject\", \"ihave\"];\nreject \"nope\";\nerror \"boom\";\n");
    let runtime = runtime();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, empty_message(), &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert_eq!(
        ctx.run(&mut handler),
        Err(RuntimeError::ScriptErrorMessage("boom".to_string()))
    );
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.rejects, handler.keeps), (1, 0));
}

#[test]
fn received_part_from_a_registered_function() {
    let mut fnc_map = FunctionMap::new().with_function_no_args("received_from", fn_received_from);
    let script = Compiler::new()
        .register_functions(&mut fnc_map)
        .compile(&add_crlf(
            b"require [\"variables\", \"vnd.stalwart.expressions\"];\n\
              let \"x\" \"received_from()\";\n\
              if not string :is \"${x}\" \"mail.example.org\" { discard; }\n",
        ))
        .unwrap();
    let raw = b"Received: from mail.example.org (mail.example.org [10.0.0.1])\r\n\
               \tby mx.example.com with ESMTP id 1; Tue, 1 Jan 2024 00:00:00 +0000\r\n\
               Subject: x\r\n\r\nbody\r\n";
    let message = MessageParser::default().parse(&raw[..]).unwrap();
    let runtime = runtime().with_functions(&mut fnc_map);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, message, &script, &mut arena);
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!((handler.keeps, handler.discards), (1, 0));
    let _ = crate::Script::Personal("root-export");
}

#[test]
fn send_message_carries_its_source() {
    let raw = b"From: sender@example.org\r\nTo: john@example.org\r\nSubject: hi\r\n\r\nbody\r\n";
    let message = MessageParser::default().parse(&raw[..]).unwrap();
    let script = compile(
        "require [\"vacation\", \"enotify\"];\n\
         vacation \"I am away\";\n\
         notify :message \"hello\" \"mailto:ops@example.org\";\n\
         redirect \"other@example.org\";\n",
    );
    let runtime = runtime()
        .with_valid_notification_uri("mailto")
        .with_max_out_messages(10);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, message, &script, &mut arena);
    ctx.set_user_address("john@example.org");
    ctx.set_envelope(Envelope::From, "sender@example.org");
    ctx.set_envelope(Envelope::To, "john@example.org");
    let mut handler = RecordingHandler::default();
    assert!(matches!(ctx.run(&mut handler), Ok(Status::Finished)));
    assert_eq!(
        handler.sends,
        [
            (MessageSource::Vacation, true),
            (MessageSource::Notification, true),
            (MessageSource::Redirect, false),
        ]
    );
}

#[test]
fn unknown_command_reports_its_position() {
    for (script, line, column) in [
        ("foo;", 1, 1),
        ("keep;\n\n  frobnicate \"x\";\n", 3, 3),
        ("if true {\n  frob;\n}\n", 2, 3),
    ] {
        let err = crate::Compiler::new()
            .compile(script.as_bytes())
            .expect_err("unknown command must not compile");
        assert_eq!(
            (err.line_num(), err.line_pos()),
            (line, column),
            "{script:?}: {err}"
        );
    }
}

#[test]
fn error_columns_are_one_based_on_every_line() {
    for (script, line, column) in [
        ("keep :bogustag;", 1, 6),
        ("\nkeep :bogustag;", 2, 6),
        ("require \"imap4flags\";\nkeep :flags \"a\" :bogus;", 2, 17),
        ("keep; }", 1, 7),
        ("\n  keep; }", 2, 9),
        ("require \"fileinto\"; fileinto 12;", 1, 30),
    ] {
        let err = crate::Compiler::new()
            .compile(script.as_bytes())
            .expect_err("script must not compile");
        assert_eq!(
            (err.line_num(), err.line_pos()),
            (line, column),
            "{script:?}: {err}"
        );
    }
}
