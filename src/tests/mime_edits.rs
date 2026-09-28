/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::add_crlf;
use crate::{
    Arena, Compiler, Context, Runtime,
    runtime::handler::{Action, Handler, Reply, Status},
};
use mail_parser::MessageParser;

const NESTED: &str = concat!(
    "From: a@example.com\r\n",
    "Subject: outer\r\n",
    "Content-Type: multipart/mixed; boundary=\"o\"\r\n",
    "\r\n",
    "--o\r\n",
    "Content-Type: text/plain\r\n",
    "\r\n",
    "first part\r\n",
    "--o\r\n",
    "Content-Type: message/rfc822\r\n",
    "Content-Transfer-Encoding: base64\r\n",
    "\r\n",
    "U3ViamVjdDogaW5uZXINCkNvbnRlbnQtVHlwZTogbXVsdGlwYXJ0L21peGVkOyBib3VuZGFyeT0iaSINCg0KLS1pDQpDb250ZW50LVR5cGU6IHRleHQvcGxhaW4NCg0KaGlkZGVuIHRyZWFzdXJlDQotLWkNCkNvbnRlbnQtVHlwZTogdGV4dC9odG1sDQoNCjxwPm1vcmU8L3A+DQotLWktLQ0K\r\n",
    "--o\r\n",
    "Content-Type: message/rfc822\r\n",
    "\r\n",
    "Subject: plain inner\r\n",
    "\r\n",
    "plain nested body\r\n",
    "--o\r\n",
    "Content-Type: text/plain\r\n",
    "\r\n",
    "last part\r\n",
    "--o--\r\n",
);

const ALTERNATIVE: &str = concat!(
    "Subject: alternative\r\n",
    "Content-Type: multipart/mixed; boundary=\"m\"\r\n",
    "\r\n",
    "--m\r\n",
    "Content-Type: multipart/alternative; boundary=\"a\"\r\n",
    "\r\n",
    "--a\r\n",
    "Content-Type: text/plain\r\n",
    "\r\n",
    "alt text\r\n",
    "--a\r\n",
    "Content-Type: text/html\r\n",
    "\r\n",
    "<p>alt html</p>\r\n",
    "--a--\r\n",
    "--m\r\n",
    "Content-Type: text/plain\r\n",
    "Content-Disposition: attachment\r\n",
    "\r\n",
    "attachment text\r\n",
    "--m--\r\n",
);

const HTML: &str = concat!(
    "Subject: html\r\n",
    "Content-Type: text/html\r\n",
    "\r\n",
    "<p>Hello <b>World</b></p>\r\n",
);

#[derive(Default)]
struct Collector {
    created: Vec<Vec<u8>>,
    duplicate_ids: Vec<String>,
}

impl<'x> Handler<'x> for Collector {
    fn action(&mut self, _: &Context<'x>, action: Action<'x>) -> Reply<()> {
        if let Action::CreatedMessage { message, .. } = action {
            self.created.push(message);
        }
        Reply::Ready(())
    }

    fn duplicate_id(&mut self, _: &Context<'x>, id: &str, _: u64, _: bool) -> Reply<bool> {
        self.duplicate_ids.push(id.to_string());
        Reply::Ready(false)
    }
}

fn run(script: &str, raw: &str) -> (String, Vec<Vec<u8>>) {
    let script = Compiler::new()
        .compile(&add_crlf(script.as_bytes()))
        .unwrap();
    let message = MessageParser::new().parse(raw.as_bytes()).unwrap();
    let runtime = Runtime::new();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, &message, &script, &mut arena);
    let mut collector = Collector::default();
    assert!(matches!(ctx.run(&mut collector), Ok(Status::Finished)));
    let result = ctx
        .global_variable("result")
        .map(|value| value.to_string().into_owned())
        .unwrap_or_default();
    (result, collector.created)
}

#[test]
fn foreverypart_skips_nested_message_parts() {
    let (result, _) = run(
        "require [\"foreverypart\", \"variables\", \"include\"];\n\
         foreverypart { set \"global.result\" \"${global.result}x\"; }\n",
        NESTED,
    );
    assert_eq!(result, "xxxxx");
}

#[test]
fn nested_foreverypart_stays_out_of_attached_messages() {
    let (result, _) = run(
        "require [\"foreverypart\", \"variables\", \"include\"];\n\
         foreverypart { foreverypart { set \"global.result\" \"${global.result}x\"; } }\n",
        NESTED,
    );
    assert_eq!(result, "xxxx");
}

#[test]
fn anychild_on_attached_message_checks_only_the_container() {
    let (result, _) = run(
        "require [\"foreverypart\", \"mime\", \"variables\", \"include\"];\n\
         foreverypart {\n\
           if header :mime :anychild :contains \"subject\" \"inner\" { set \"global.result\" \"${global.result}x\"; }\n\
         }\n",
        NESTED,
    );
    assert_eq!(result, "");
}

#[test]
fn body_test_enters_nested_messages() {
    let (result, _) = run(
        "require [\"body\", \"variables\", \"include\"];\n\
         if body :text :contains \"hidden treasure\" { set \"global.result\" \"encoded\"; }\n\
         if body :text :contains \"plain nested body\" { set \"global.result\" \"${global.result} plain\"; }\n\
         if body :text :contains \"hidden treasure\" { set \"global.result\" \"${global.result} cached\"; }\n",
        NESTED,
    );
    assert_eq!(result, "encoded plain cached");
}

#[test]
fn replace_inside_foreverypart_hides_children() {
    let (result, created) = run(
        "require [\"foreverypart\", \"mime\", \"replace\", \"variables\", \"include\"];\n\
         foreverypart {\n\
           set \"global.result\" \"${global.result}x\";\n\
           if header :mime :subtype \"content-type\" \"alternative\" { replace \"replaced alternative\"; }\n\
         }\n",
        ALTERNATIVE,
    );
    assert_eq!(result, "xxx");
    let [created] = created.as_slice() else {
        panic!("expected one created message, got {}", created.len());
    };
    let created = std::str::from_utf8(created).unwrap();
    assert!(created.contains("replaced alternative"), "{created}");
    assert!(!created.contains("alt html"), "{created}");
    assert!(created.contains("attachment text"), "{created}");
    assert!(created.ends_with("--m--\r\n"), "{created}");
}

#[test]
fn enclose_is_visible_to_later_tests() {
    let (result, created) = run(
        "require [\"enclose\", \"body\", \"variables\", \"include\"];\n\
         enclose :subject \"wrapped\" \"see attached\";\n\
         if header :is \"subject\" \"wrapped\" { set \"global.result\" \"subject\"; }\n\
         if body :text :contains \"first part\" { set \"global.result\" \"${global.result} body\"; }\n",
        NESTED,
    );
    assert_eq!(result, "subject body");
    let [created] = created.as_slice() else {
        panic!("expected one created message, got {}", created.len());
    };
    let created = std::str::from_utf8(created).unwrap();
    assert!(
        created.starts_with("Content-Type: multipart/mixed;"),
        "{created}"
    );
    assert!(created.contains(NESTED), "{created}");
}

#[test]
fn convert_is_visible_to_body_test() {
    let (result, created) = run(
        "require [\"convert\", \"body\", \"variables\", \"include\"];\n\
         if body :content \"text/plain\" :contains \"World\" { set \"global.result\" \"before\"; }\n\
         convert \"text/html\" \"text/plain\" \"\";\n\
         if body :content \"text/plain\" :contains \"World\" { set \"global.result\" \"${global.result}after\"; }\n\
         if body :raw :contains \"<b>\" { set \"global.result\" \"${global.result} raw\"; }\n",
        HTML,
    );
    assert_eq!(result, "after");
    let [created] = created.as_slice() else {
        panic!("expected one created message, got {}", created.len());
    };
    let created = std::str::from_utf8(created).unwrap();
    assert!(
        created.starts_with("Content-Type: text/plain; charset=utf8\r\n\r\n"),
        "{created}"
    );
    assert!(!created.contains("<b>"), "{created}");
}

#[test]
fn addheader_keeps_untouched_bytes() {
    let (_, created) = run(
        "require [\"editheader\"];\n\
         addheader \"X-New\" \"yes\";\n",
        NESTED,
    );
    let [created] = created.as_slice() else {
        panic!("expected one created message, got {}", created.len());
    };
    assert_eq!(
        created.as_slice(),
        format!("X-New: yes\r\n{NESTED}").as_bytes()
    );
}

#[test]
fn deleteheader_on_nested_part_keeps_siblings() {
    let (result, created) = run(
        "require [\"foreverypart\", \"mime\", \"editheader\", \"variables\", \"include\"];\n\
         foreverypart {\n\
           if header :mime :type \"content-type\" \"message\" { deleteheader \"Content-Transfer-Encoding\"; }\n\
         }\n\
         if header :mime :anychild :contains \"content-transfer-encoding\" \"base64\" { set \"global.result\" \"kept\"; }\n",
        NESTED,
    );
    assert_eq!(result, "");
    let [created] = created.as_slice() else {
        panic!("expected one created message, got {}", created.len());
    };
    let expected = NESTED.replacen("Content-Transfer-Encoding: base64\r\n", "", 1);
    assert_eq!(std::str::from_utf8(created).unwrap(), expected);
}

#[test]
fn repeated_body_tests_after_convert_keep_one_copy() {
    let body = "plain words ".repeat(10_000);
    let raw = format!("Subject: big\r\nContent-Type: text/plain\r\n\r\n{body}\r\n");
    let mut script = String::from(
        "require [\"convert\", \"body\"];\nconvert \"text/plain\" \"text/html\" \"\";\n",
    );
    for _ in 0..200 {
        script.push_str("if body :text :contains \"zzz\" { stop; }\n");
    }
    let script = Compiler::new()
        .compile(&add_crlf(script.as_bytes()))
        .unwrap();
    let message = MessageParser::new().parse(raw.as_bytes()).unwrap();
    let runtime = Runtime::new().with_memory_limit(4 << 20);
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, &message, &script, &mut arena);
    assert!(matches!(
        ctx.run(&mut Collector::default()),
        Ok(Status::Finished)
    ));
    drop(ctx);
    assert!(
        arena.allocated_bytes() < 2 << 20,
        "{}",
        arena.allocated_bytes()
    );
}

#[test]
fn charged_text_respects_the_memory_limit() {
    let mut arena = Arena::new();
    arena.prepare(64 << 10);
    assert_eq!(
        arena.keep_charged_text("x".repeat(1000)),
        Some("x".repeat(1000).as_str())
    );
    assert_eq!(arena.keep_charged_text("y".repeat(128 << 10)), None);
    assert!(arena.bump.try_alloc_slice_fill_copy(64 << 10, 0u8).is_err());
    assert!(arena.allocated_bytes() >= 1000);
    arena.prepare(64 << 10);
    assert!(arena.keep_charged_text("z".repeat(32 << 10)).is_some());
}

#[test]
fn duplicate_header_takes_a_single_message_id() {
    let raw = concat!(
        "Message-ID: <single@example.com>\r\n",
        "References: <r1@example.com> <r2@example.com>\r\n",
        "Subject: ids\r\n",
        "\r\n",
        "body\r\n",
    );
    let script = Compiler::new()
        .compile(&add_crlf(
            b"require [\"duplicate\"];\n\
              if duplicate :header \"References\" { stop; }\n\
              if duplicate :header \"Message-ID\" { stop; }\n",
        ))
        .unwrap();
    let message = MessageParser::new().parse(raw.as_bytes()).unwrap();
    let runtime = Runtime::new();
    let mut arena = Arena::new();
    let mut ctx = Context::new(&runtime, &message, &script, &mut arena);
    let mut collector = Collector::default();
    assert!(matches!(ctx.run(&mut collector), Ok(Status::Finished)));
    assert_eq!(collector.duplicate_ids, ["single@example.com"]);
}

#[test]
fn context_and_arena_are_send() {
    fn assert_send<T: Send>() {}
    assert_send::<Context<'static>>();
    assert_send::<Arena>();
}
