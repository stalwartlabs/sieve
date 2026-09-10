# sieve

[![crates.io](https://img.shields.io/crates/v/sieve-rs)](https://crates.io/crates/sieve-rs)
[![build](https://github.com/stalwartlabs/sieve/actions/workflows/rust.yml/badge.svg)](https://github.com/stalwartlabs/sieve/actions/workflows/rust.yml)
[![docs.rs](https://img.shields.io/docsrs/sieve-rs)](https://docs.rs/sieve-rs)
[![License: AGPL v3](https://img.shields.io/badge/License-AGPL_v3-blue.svg)](https://www.gnu.org/licenses/agpl-3.0)

_sieve_ is a fast and secure Sieve filter interpreter for Rust that supports all [registered Sieve extensions](https://www.iana.org/assignments/sieve-extensions/sieve-extensions.xhtml).

## Usage Example

```rust
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
```

Scripts compile to a compact bytecode. Store `script.to_bytes()` and load it again
with `Sieve::from_bytes(&bytes)`, which borrows the buffer instead of copying it, so a
script can be loaded on every delivery at almost no cost. Operations that need an
asynchronous answer (list lookups, duplicate tracking, mailbox checks, external
functions, scripts loaded on demand) return `Reply::Pending` from the handler; the
interpreter then returns `Status::Pending` and continues after `resume(input)`.
Scripts loaded while a run is in progress (`include` of a user script) must outlive
the `Context`; push them into a `ScriptArena` declared before the context and hand
the interpreter the reference it returns.

## Testing & Fuzzing

To run the testsuite:

```bash
 $ cargo test --all-features
```

To fuzz the library with `cargo-fuzz`:

```bash
 $ cargo +nightly fuzz run sieve
```

## Conformed RFCs

- [RFC 5228 - Sieve: An Email Filtering Language](https://datatracker.ietf.org/doc/html/rfc5228)
- [RFC 3894 - Copying Without Side Effects](https://datatracker.ietf.org/doc/html/rfc3894)
- [RFC 5173 - Body Extension](https://datatracker.ietf.org/doc/html/rfc5173)
- [RFC 5183 - Environment Extension](https://datatracker.ietf.org/doc/html/rfc5183)
- [RFC 5229 - Variables Extension](https://datatracker.ietf.org/doc/html/rfc5229)
- [RFC 5230 - Vacation Extension](https://datatracker.ietf.org/doc/html/rfc5230)
- [RFC 5231 - Relational Extension](https://datatracker.ietf.org/doc/html/rfc5231)
- [RFC 5232 - Imap4flags Extension](https://datatracker.ietf.org/doc/html/rfc5232)
- [RFC 5233 - Subaddress Extension](https://datatracker.ietf.org/doc/html/rfc5233)
- [RFC 5235 - Spamtest and Virustest Extensions](https://datatracker.ietf.org/doc/html/rfc5235)
- [RFC 5260 - Date and Index Extensions](https://datatracker.ietf.org/doc/html/rfc5260)
- [RFC 5293 - Editheader Extension](https://datatracker.ietf.org/doc/html/rfc5293)
- [RFC 5429 - Reject and Extended Reject Extensions](https://datatracker.ietf.org/doc/html/rfc5429)
- [RFC 5435 - Extension for Notifications](https://datatracker.ietf.org/doc/html/rfc5435)
- [RFC 5463 - Ihave Extension](https://datatracker.ietf.org/doc/html/rfc5463)
- [RFC 5490 - Extensions for Checking Mailbox Status and Accessing Mailbox Metadata](https://datatracker.ietf.org/doc/html/rfc5490)
- [RFC 5703 - MIME Part Tests, Iteration, Extraction, Replacement, and Enclosure](https://datatracker.ietf.org/doc/html/rfc5703)
- [RFC 6009 - Delivery Status Notifications and Deliver-By Extensions](https://datatracker.ietf.org/doc/html/rfc6009)
- [RFC 6131 - Sieve Vacation Extension: "Seconds" Parameter](https://datatracker.ietf.org/doc/html/rfc6131)
- [RFC 6134 - Externally Stored Lists](https://datatracker.ietf.org/doc/html/rfc6134)
- [RFC 6558 - Converting Messages before Delivery](https://datatracker.ietf.org/doc/html/rfc6558)
- [RFC 6609 - Include Extension](https://datatracker.ietf.org/doc/html/rfc6609)
- [RFC 7352 - Detecting Duplicate Deliveries](https://datatracker.ietf.org/doc/html/rfc7352)
- [RFC 8579 - Delivering to Special-Use Mailboxes](https://datatracker.ietf.org/doc/html/rfc8579)
- [RFC 8580 - File Carbon Copy (FCC)](https://datatracker.ietf.org/doc/html/rfc8580)
- [RFC 9042 - Delivery by MAILBOXID](https://datatracker.ietf.org/doc/html/rfc9042)
- [REGEX-01 - Regular Expression Extension (draft-ietf-sieve-regex-01)](https://www.ietf.org/archive/id/draft-ietf-sieve-regex-01.html)

## License

Licensed under the terms of the [GNU Affero General Public License](https://www.gnu.org/licenses/agpl-3.0.en.html) as published by
the Free Software Foundation, either version 3 of the License, or (at your option) any later version.

You can be released from the requirements of the AGPLv3 license by purchasing a commercial license. Please contact licensing@stalw.art for more details.
  
## Copyright

Copyright (C) 2020, Stalwart Labs LLC
