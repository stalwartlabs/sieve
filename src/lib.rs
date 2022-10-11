use std::{borrow::Cow, sync::Arc, vec::IntoIter};

use ahash::{AHashMap, AHashSet};
use compiler::grammar::{instruction::Instruction, Capability};
use mail_parser::{HeaderName, Message};
use runtime::{
    actions::{action_editheader::InsertHeader, action_mime::ReplacePart},
    context::ScriptStack,
};
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

#[derive(Clone)]
pub struct Context<'x> {
    #[cfg(test)]
    pub(crate) runtime: Runtime,
    #[cfg(not(test))]
    pub(crate) runtime: &'x Runtime,
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

    pub(crate) header_insertions: Vec<InsertHeader<'x>>,
    pub(crate) header_deletions: Vec<usize>,
    pub(crate) part_replacements: Vec<ReplacePart<'x>>,
    pub(crate) part_deletions: Vec<usize>,
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

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event {
    IncludeScript {
        name: Script,
    },
    MailboxExists {
        names: Vec<String>,
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
    use std::{fs, path::PathBuf};

    use ahash::AHashMap;
    use mail_parser::{Encoding, Message, MessagePart, PartType};

    use crate::{Compiler, Context, Event, Input, Runtime};

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
        let compiler = Compiler::new();
        let script_path = PathBuf::from("tests/extensions/mime/execute.svtest");
        let mut ancestors = script_path.ancestors();
        ancestors.next();
        let base_path = ancestors.next().unwrap();
        let script = compiler.compile(&fs::read(&script_path).unwrap()).unwrap();

        let runtime = Runtime::new()
            .with_protected_header("Auto-Submitted")
            .with_protected_header("Received");
        let mut instance = runtime.filter(b"");
        let mut input = Input::script("", script);
        let mut current_test = String::new();
        let mut actions = Vec::new();
        let mut raw_message = Vec::new();

        'outer: loop {
            let raw_message_ = raw_message.clone();
            instance.message = Message::parse(&raw_message_).unwrap_or_else(|| Message {
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

            while let Some(event) = instance.run(input) {
                match event.unwrap() {
                    Event::IncludeScript { name } => {
                        let mut include_path = PathBuf::from(base_path);
                        include_path.push("included");
                        include_path.push(format!("{}.sieve", name));
                        let script = compiler
                            .compile(&fs::read(include_path.as_path()).unwrap())
                            .unwrap();
                        input = Input::script(name, script);
                    }
                    Event::MailboxExists { names } => {
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
                                    instance.part = 0;
                                    instance.part_iter = vec![].into_iter();
                                    instance.part_iter_stack = Vec::new();
                                    let raw_message_ = params.pop().unwrap();
                                    raw_message = Vec::with_capacity(raw_message_.len());
                                    let mut last_ch = 0;
                                    for &ch in raw_message_.as_bytes() {
                                        if ch == b'\n' && last_ch != b'\r' {
                                            raw_message.push(b'\r');
                                        }
                                        raw_message.push(ch);
                                        last_ch = ch;
                                    }
                                    instance.message_size = raw_message.len();
                                    instance.header_deletions.clear();
                                    instance.header_insertions.clear();
                                    instance.part_deletions.clear();
                                    instance.part_replacements.clear();
                                    instance = Context {
                                        runtime: self.runtime,
                                        message: Message {
                                            html_body: vec![],
                                            text_body: vec![],
                                            attachments: vec![],
                                            parts: vec![],
                                            raw_message: b""[..].into(),
                                        },
                                        envelope: self.envelope,
                                        part: self.part,
                                        part_iter: self.part_iter,
                                        part_iter_stack: self.part_iter_stack,
                                        message_size: self.message_size,
                                        pos: self.pos,
                                        test_result: self.test_result,
                                        script_cache: self.script_cache,
                                        script_stack: self.script_stack,
                                        vars_global: self.vars_global,
                                        vars_local: self.vars_local,
                                        vars_match: self.vars_match,
                                        header_insertions: self.header_insertions,
                                        header_deletions: self.header_deletions,
                                        part_replacements: self.part_replacements,
                                        part_deletions: self.part_deletions,
                                    };
                                    continue 'outer;
                                } else if let Some(envelope) = target.strip_prefix("envelope.") {
                                    instance.clear_envelope();
                                    instance
                                        .set_envelope(envelope.to_string(), &params.pop().unwrap());
                                } else {
                                    panic!("test_set {} not implemented.", target);
                                }
                            }
                            "test_message" => {
                                let mut params = params.into_iter();
                                input = match params.next().unwrap().as_str() {
                                    ":folder" => {
                                        let folder_name = params.next().expect("test_message folder name");
                                        (actions.is_empty() && folder_name.eq_ignore_ascii_case("INBOX")) || 
                                        actions.iter().any(|a| matches!(a, Event::FileInto { folder, .. } if folder == &folder_name ))
                                    }
                                    ":smtp" => {
                                        actions.iter().any(|a| matches!(a, Event::Redirect { .. } ))
                                    }
                                    param => panic!("Invalid test_message param '{}'", param),
                                }.into();
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
                                input = (actions.is_empty()
                                    || actions.iter().any(|a| {
                                        matches!(a, Event::FileInto { .. } | Event::Redirect { .. })
                                    }))
                                .into();
                            }
                            "test_result_reset" => {
                                actions.clear();
                                instance.header_deletions.clear();
                                instance.header_insertions.clear();
                                instance.part_deletions.clear();
                                instance.part_replacements.clear();
                            }
                            "test_config_reload" => (),
                            "test_fail" => {
                                panic!("Test '{}' failed: {}", current_test, params.pop().unwrap());
                            }
                            _ => panic!("Test command {} not implemented.", command),
                        }
                    }
                    action => {
                        actions.push(action);
                        input = Input::True;
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
}
