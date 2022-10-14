use std::{borrow::Cow, sync::Arc, vec::IntoIter};

use ahash::{AHashMap, AHashSet};
use compiler::grammar::{instruction::Instruction, Capability};
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
    pub(crate) max_script_len: usize,
    pub(crate) max_string_len: usize,
    pub(crate) max_variable_len: usize,
    pub(crate) max_nested_blocks: usize,
    pub(crate) max_nested_tests: usize,
    pub(crate) max_match_variables: usize,
    pub(crate) max_local_variables: usize,
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub(crate) allowed_capabilities: AHashSet<Capability>,
    pub(crate) protected_headers: Vec<HeaderName<'static>>,
    pub(crate) environment: AHashMap<String, String>,
    pub(crate) include_scripts: AHashMap<String, Arc<Sieve>>,

    pub(crate) max_include_scripts: usize,
    pub(crate) max_instructions: usize,
}

#[derive(Clone, Debug)]
pub struct Context<'x> {
    #[cfg(test)]
    pub(crate) runtime: Runtime,
    #[cfg(not(test))]
    pub(crate) runtime: &'x Runtime,
    pub(crate) default_from: Cow<'x, str>,
    pub(crate) message: Message<'x>,
    pub(crate) envelope: Vec<(Envelope<'x>, String)>,
    pub(crate) part: usize,
    pub(crate) part_iter: IntoIter<usize>,
    pub(crate) part_iter_stack: Vec<(usize, IntoIter<usize>)>,
    pub(crate) message_size: usize,

    pub(crate) pos: usize,
    pub(crate) test_result: bool,
    pub(crate) script_cache: AHashMap<Script, Arc<Sieve>>,
    pub(crate) script_stack: Vec<ScriptStack>,
    pub(crate) vars_global: AHashMap<String, String>,
    pub(crate) vars_local: Vec<String>,
    pub(crate) vars_match: Vec<String>,

    pub(crate) actions: Vec<Action>,
    pub(crate) has_changes: bool,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Script {
    Personal(String),
    Global(String),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Envelope<'x> {
    From,
    To,
    ByTimeAbsolute,
    ByTimeRelative,
    ByMode,
    ByTrace,
    Other(Cow<'x, str>),
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Action {
    Keep {
        flags: Vec<String>,
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
        copy: bool,
        create: bool,
    },
    Redirect {
        address: String,
        copy: bool,
    },
    UpdateMessage {
        bytes: Vec<u8>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event {
    IncludeScript {
        name: Script,
    },
    MailboxExists {
        names: Vec<String>,
    },

    #[cfg(test)]
    TestCommand {
        command: String,
        params: Vec<String>,
    },
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Input {
    True,
    False,
    Script { name: Script, script: Arc<Sieve> },
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::{Path, PathBuf},
    };

    use mail_parser::{Encoding, Message, MessagePart, PartType};

    use crate::{Action, Compiler, Envelope, Event, Input, Runtime, runtime::actions::action_mime::reset_test_boundary};

    /*fn read_dir(path: PathBuf, files: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap().path();
            if entry.is_dir() {
                read_dir(entry, files);
            } else if ["svtest", "sieve"]
                .contains(&entry.extension().and_then(|e| e.to_str()).unwrap_or(""))
            {
                files.push(entry);
            }
        }
    }*/

    #[test]
    fn test_suite() {
        for test in [
            /*"tests/execute/mailstore.svtest",
            "tests/execute/actions.svtest",
            "tests/execute/smtp.svtest",
            "tests/execute/errors-cpu-limit.svtest",
            "tests/execute/address-normalize.svtest",
            "tests/execute/examples.svtest",
            "tests/execute/errors.svtest",
            "tests/plugins/extprograms/execute/command.svtest",
            "tests/plugins/extprograms/execute/execute.svtest",
            "tests/plugins/extprograms/execute/errors.svtest",
            "tests/plugins/extprograms/pipe/command.svtest",
            "tests/plugins/extprograms/pipe/execute.svtest",
            "tests/plugins/extprograms/pipe/errors.svtest",
            "tests/plugins/extprograms/filter/command.svtest",
            "tests/plugins/extprograms/filter/execute.svtest",
            "tests/plugins/extprograms/filter/errors.svtest",
            "tests/plugins/extprograms/errors.svtest",
            "tests/compile/recover.svtest",
            "tests/compile/compile.svtest",
            "tests/compile/warnings.svtest",
            "tests/compile/errors.svtest",
            "tests/extensions/imap4flags/multiscript.svtest",
            "tests/extensions/imap4flags/hasflag.svtest",
            "tests/extensions/imap4flags/flagstring.svtest",
            "tests/extensions/imap4flags/basic.svtest",
            "tests/extensions/imap4flags/flagstore.svtest",
            "tests/extensions/imap4flags/execute.svtest",
            "tests/extensions/imap4flags/errors.svtest",
            "tests/extensions/mailbox/execute.svtest",
            "tests/extensions/mailbox/errors.svtest",
            "tests/extensions/vnd.dovecot/report/execute.svtest",
            "tests/extensions/vnd.dovecot/report/errors.svtest",
            "tests/extensions/vnd.dovecot/debug/execute.svtest",
            "tests/extensions/vnd.dovecot/environment/basic.svtest",
            "tests/extensions/vnd.dovecot/environment/variables.svtest",
            "tests/extensions/metadata/execute.svtest",
            "tests/extensions/metadata/errors.svtest",
            "tests/extensions/special-use/execute.svtest",
            "tests/extensions/special-use/errors.svtest",
            "tests/extensions/reject/smtp.svtest",
            "tests/extensions/reject/execute.svtest",
            "tests/extensions/enotify/notify_method_capability.svtest",
            "tests/extensions/enotify/mailto.svtest",
            "tests/extensions/enotify/encodeurl.svtest",
            "tests/extensions/enotify/basic.svtest",
            "tests/extensions/enotify/valid_notify_method.svtest",
            "tests/extensions/enotify/execute.svtest",
            "tests/extensions/enotify/errors.svtest",
            "tests/extensions/ihave/restrictions.svtest",
            "tests/extensions/ihave/execute.svtest",
            "tests/extensions/ihave/errors.svtest",
            "tests/extensions/vacation/reply.svtest",
            "tests/extensions/vacation/message.svtest",
            "tests/extensions/vacation/smtp.svtest",
            "tests/extensions/vacation/execute.svtest",
            "tests/extensions/vacation/utf-8.svtest",
            "tests/extensions/vacation/errors.svtest",
            "tests/extensions/include/twice.svtest",
            "tests/extensions/include/rfc.svtest",
            "tests/extensions/include/once.svtest",
            "tests/extensions/include/optional.svtest",
            "tests/extensions/include/execute.svtest",
            "tests/extensions/include/variables.svtest",
            "tests/extensions/include/errors.svtest",
            "tests/extensions/spamvirustest/spamtest.svtest",
            "tests/extensions/spamvirustest/virustest.svtest",
            "tests/extensions/spamvirustest/spamtestplus.svtest",
            "tests/extensions/spamvirustest/errors.svtest",
            "tests/extensions/duplicate/execute.svtest",
            "tests/extensions/duplicate/errors.svtest",
            "tests/extensions/variables/errors.svtest",
            "tests/extensions/editheader/errors.svtest",
            "tests/extensions/mime/errors.svtest",
            "tests/extensions/date/zones.svtest",
            "tests/extensions/date/basic.svtest",
            "tests/extensions/date/date-parts.svtest",
            "tests/extensions/regex/errors.svtest",
            "tests/extensions/relational/errors.svtest",
            "tests/extensions/body/errors.svtest",
            "tests/extensions/index/errors.svtest",
            "tests/failures/fuzz1.svtest",
            "tests/failures/fuzz2.svtest",
            "tests/failures/mailbox-bad-utf8.svtest",
            "tests/failures/fuzz3.svtest",
            "tests/multiscript/conflicts.svtest",
            "tests/multiscript/basic.svtest",
            "tests/extensions/subaddress/config.svtest",
            "tests/extensions/variables/limits.svtest",
            "tests/extensions/environment/rfc.svtest",
            "tests/extensions/environment/basic.svtest",
            "tests/extensions/index/basic.svtest",

            */
            "tests/test-size.svtest",
            "tests/test-anyof.svtest",
            "tests/test-allof.svtest",
            "tests/test-exists.svtest",
            "tests/control-stop.svtest",
            "tests/test-address.svtest",
            "tests/control-if.svtest",
            "tests/lexer.svtest",
            "tests/testsuite.svtest",
            "tests/test-header.svtest",
            "tests/match-types/is.svtest",
            "tests/match-types/contains.svtest",
            "tests/match-types/matches.svtest",
            "tests/comparators/i-octet.svtest",
            "tests/comparators/i-ascii-casemap.svtest",
            "tests/extensions/subaddress/rfc.svtest",
            "tests/extensions/subaddress/basic.svtest",
            "tests/extensions/encoded-character.svtest",
            "tests/extensions/variables/string.svtest",
            "tests/extensions/variables/match.svtest",
            "tests/extensions/variables/regex.svtest",
            "tests/extensions/variables/basic.svtest",
            "tests/extensions/variables/modifiers.svtest",
            "tests/extensions/variables/quoting.svtest",
            "tests/extensions/editheader/utf8.svtest",
            "tests/extensions/editheader/protected.svtest",
            "tests/extensions/editheader/addheader.svtest",
            "tests/extensions/editheader/deleteheader.svtest",
            "tests/extensions/editheader/execute.svtest",
            "tests/extensions/editheader/alternating.svtest",
            "tests/extensions/mime/header.svtest",
            "tests/extensions/mime/calendar-example.svtest",
            "tests/extensions/mime/address.svtest",
            "tests/extensions/mime/extracttext.svtest",
            "tests/extensions/mime/content-header.svtest",
            "tests/extensions/mime/execute.svtest",
            "tests/extensions/mime/foreverypart.svtest",
            "tests/extensions/mime/exists.svtest",
            "tests/extensions/mime/replace.svtest", 
            "tests/extensions/mime/enclose.svtest", 
            "tests/extensions/regex/match-values.svtest",
            "tests/extensions/regex/basic.svtest",
            "tests/extensions/relational/rfc.svtest",
            "tests/extensions/relational/comparators.svtest",
            "tests/extensions/relational/basic.svtest",
            "tests/extensions/envelope.svtest",
            "tests/extensions/body/content.svtest",
            "tests/extensions/body/text.svtest",
            "tests/extensions/body/match-values.svtest",
            "tests/extensions/body/basic.svtest",
            "tests/extensions/body/raw.svtest",
        ] {
            println!("===== {} =====", test);
            run_test(&PathBuf::from(test));
        }
    }

    fn run_test(script_path: &Path) {
        let compiler = Compiler::new();
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

        'outer: loop {
            let runtime = Runtime::new()
                .with_protected_header("Auto-Submitted")
                .with_protected_header("Received");
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

            while let Some(event) = instance.run(input) {
                match event.unwrap() {
                    Event::IncludeScript { name } => {
                        let mut include_path = PathBuf::from(base_path);
                        include_path.push("included");
                        include_path.push(format!("{}.sieve", name));
                        let script = compiler
                            .compile(&add_crlf(&fs::read(include_path.as_path()).unwrap()))
                            .unwrap();
                        input = Input::script(name, script);
                    }
                    Event::MailboxExists { .. } => {
                        input = Input::True;
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
                                let target = params.first().expect("test_set parameter");
                                if target == "message" {
                                    raw_message_ = params.pop().unwrap().into_bytes().into();
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
                                    let envelope = Envelope::from(envelope.to_string());
                                    instance.envelope.retain(|(e, _)| e != &envelope);
                                    instance.set_envelope(envelope, &params.pop().unwrap());
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
                                        instance.actions.iter().any(|a| matches!(a, Action::Redirect { .. } ))
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
                                match params.next().unwrap().as_str() {
                                    "sieve_editheader_protected"
                                    | "sieve_editheader_forbid_add"
                                    | "sieve_editheader_forbid_delete" => {
                                        let value = params.next().expect("test_config_set value");
                                        if !value.is_empty() {
                                            for header_name in value.split(' ') {
                                                instance
                                                    .runtime
                                                    .add_protected_header(header_name.to_string());
                                            }
                                        } else {
                                            instance.runtime.protected_headers.clear();
                                        }
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
                                            | Action::Redirect { .. }
                                    )
                                }))
                                .into();
                            }
                            "test_result_reset" => {
                                instance.actions = vec![Action::Keep { flags: vec![] }];
                                instance.has_changes = false;
                                reset_test_boundary();
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
