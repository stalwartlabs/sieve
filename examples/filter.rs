/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use sieve::{Arena, Compiler, Context, Handler, Reply, Runtime, SieveAction, Status};

struct Printer {
    messages: Vec<String>,
    raw_message: &'static str,
}

impl<'x> Handler<'x> for Printer {
    fn action(&mut self, _: &Context<'x>, action: SieveAction<'_>) -> Reply<()> {
        match action {
            SieveAction::Keep { flags, message_id } => {
                println!(
                    "Keep message '{}' with flags {:?}.",
                    self.message(message_id),
                    flags
                );
            }
            SieveAction::Discard => {
                println!("Discard message.");
            }
            SieveAction::Reject { reason, .. } => {
                println!("Reject message with reason {reason:?}.");
            }
            SieveAction::FileInto {
                folder,
                flags,
                message_id,
                ..
            } => {
                println!(
                    "File message '{}' in folder {:?} with flags {:?}.",
                    self.message(message_id),
                    folder,
                    flags
                );
            }
            SieveAction::SendMessage {
                recipient,
                message_id,
                ..
            } => {
                println!(
                    "Send message '{}' to {:?}.",
                    self.message(message_id),
                    recipient
                );
            }
            SieveAction::Notify {
                message, method, ..
            } => {
                println!("Notify URI {method:?} with message {message:?}");
            }
            SieveAction::SetEnvelope { envelope, value } => {
                println!("Set envelope {envelope:?} to {value:?}");
            }
            SieveAction::CreatedMessage { message, .. } => {
                self.messages
                    .push(String::from_utf8_lossy(&message).into_owned());
            }
        }
        Reply::Ready(())
    }
}

impl Printer {
    fn message(&self, message_id: usize) -> &str {
        if message_id > 0 {
            self.messages[message_id - 1].as_str()
        } else {
            self.raw_message
        }
    }
}

fn main() {
    let text_script = br#"
    require ["fileinto", "body", "imap4flags"];
    
    if body :contains "tps" {
        setflag "$tps_reports";
    }

    if header :matches "List-ID" "*<*@*" {
        fileinto "INBOX.lists.${2}"; stop;
    }
    "#;
    let raw_message = r#"From: Sales Mailing List <list-sales@example.org>
To: John Doe <jdoe@example.org>
List-ID: <sales@example.org>
Subject: TPS Reports

We're putting new coversheets on all the TPS reports before they go out now.
So if you could go ahead and try to remember to do that from now on, that'd be great. All right! 
"#;

    let compiler = Compiler::new();
    let script = compiler.compile(text_script).unwrap();

    let runtime = Runtime::new();

    let mut arena = Arena::new();
    let mut instance = runtime.filter(raw_message.as_bytes(), &script, &mut arena);
    let mut handler = Printer {
        messages: Vec::new(),
        raw_message,
    };

    loop {
        match instance.run(&mut handler) {
            Ok(Status::Finished) => break,
            Ok(Status::Pending) => instance.resume(true),
            Err(error) => {
                println!("Runtime error {error}");
                break;
            }
        }
    }
}
