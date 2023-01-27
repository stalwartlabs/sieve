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
                        panic!("Script {} not found.", name);
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
                Event::Execute { command, arguments } => {
                    println!(
                        "Script executed command {:?} with parameters {:?}",
                        command, arguments
                    );
                    // Set to true if the script succeeded
                    input = false.into();
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
                    println!("Reject message with reason {:?}.", reason);
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
                    println!("Notify URI {:?} with message {:?}", method, message);
                    input = true.into();
                }
                Event::CreatedMessage { message, .. } => {
                    messages.push(String::from_utf8(message).unwrap());
                    input = true.into();
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
                        eprintln!("Script called the 'error' function with {:?}", message);
                    }
                    RuntimeError::CapabilityNotAllowed(capability) => {
                        eprintln!(
                            "Capability {:?} has been disabled by the administrator.",
                            capability
                        );
                    }
                    RuntimeError::CapabilityNotSupported(capability) => {
                        eprintln!("Capability {:?} not supported.", capability);
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
