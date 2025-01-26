/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use sieve::{runtime::RuntimeError, Compiler, Event, Input, Runtime};

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

    // Compile
    let compiler = Compiler::new();
    let script = compiler.compile(text_script).unwrap();

    // Build runtime
    let runtime = Runtime::new();

    // Create filter instance
    let mut instance = runtime.filter(raw_message.as_bytes());
    let mut input = Input::script("my-script", script);
    let mut messages: Vec<String> = Vec::new();

    // Start event loop
    while let Some(result) = instance.run(input) {
        match result {
            Ok(event) => match event {
                Event::IncludeScript { name, optional } => {
                    // NOTE: Just for demonstration purposes, script name needs to be validated first.
                    if let Ok(bytes) = std::fs::read(name.as_str()) {
                        let script = compiler.compile(&bytes).unwrap();
                        input = Input::script(name, script);
                    } else if optional {
                        input = Input::False;
                    } else {
                        panic!("Script {name} not found.");
                    }
                }
                Event::MailboxExists { .. } => {
                    // Set to true if the mailbox exists
                    input = false.into();
                }
                Event::ListContains { .. } => {
                    // Set to true if the list(s) contains an entry
                    input = false.into();
                }
                Event::DuplicateId { .. } => {
                    // Set to true if the ID is duplicate
                    input = false.into();
                }
                Event::SetEnvelope { envelope, value } => {
                    println!("Set envelope {envelope:?} to {value:?}");
                    input = true.into();
                }

                Event::Keep { flags, message_id } => {
                    println!(
                        "Keep message '{}' with flags {:?}.",
                        if message_id > 0 {
                            messages[message_id - 1].as_str()
                        } else {
                            raw_message
                        },
                        flags
                    );
                    input = true.into();
                }
                Event::Discard => {
                    println!("Discard message.");
                    input = true.into();
                }
                Event::Reject { reason, .. } => {
                    println!("Reject message with reason {reason:?}.");
                    input = true.into();
                }
                Event::FileInto {
                    folder,
                    flags,
                    message_id,
                    ..
                } => {
                    println!(
                        "File message '{}' in folder {:?} with flags {:?}.",
                        if message_id > 0 {
                            messages[message_id - 1].as_str()
                        } else {
                            raw_message
                        },
                        folder,
                        flags
                    );
                    input = true.into();
                }
                Event::SendMessage {
                    recipient,
                    message_id,
                    ..
                } => {
                    println!(
                        "Send message '{}' to {:?}.",
                        if message_id > 0 {
                            messages[message_id - 1].as_str()
                        } else {
                            raw_message
                        },
                        recipient
                    );
                    input = true.into();
                }
                Event::Notify {
                    message, method, ..
                } => {
                    println!("Notify URI {method:?} with message {message:?}");
                    input = true.into();
                }
                Event::CreatedMessage { message, .. } => {
                    messages.push(String::from_utf8(message).unwrap());
                    input = true.into();
                }
                Event::Function { id, arguments } => {
                    println!(
                        "Script executed external function {id} with parameters {arguments:?}"
                    );
                    // Return variable result back to interpreter
                    input = Input::result("hello world".into());
                }

                #[cfg(test)]
                _ => unreachable!(),
            },
            Err(error) => {
                match error {
                    RuntimeError::TooManyIncludes => {
                        eprintln!("Too many included scripts.");
                    }
                    RuntimeError::InvalidInstruction(instruction) => {
                        eprintln!(
                            "Invalid instruction {:?} found at {}:{}.",
                            instruction.name(),
                            instruction.line_num(),
                            instruction.line_pos()
                        );
                    }
                    RuntimeError::ScriptErrorMessage(message) => {
                        eprintln!("Script called the 'error' function with {message:?}");
                    }
                    RuntimeError::CapabilityNotAllowed(capability) => {
                        eprintln!(
                            "Capability {capability:?} has been disabled by the administrator.",
                        );
                    }
                    RuntimeError::CapabilityNotSupported(capability) => {
                        eprintln!("Capability {capability:?} not supported.");
                    }
                    RuntimeError::CPULimitReached => {
                        eprintln!("Script exceeded the configured CPU limit.");
                    }
                }
                input = true.into();
            }
        }
    }
}
