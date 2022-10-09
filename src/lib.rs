use std::{borrow::Cow, sync::Arc, vec::IntoIter};

use ahash::{AHashMap, AHashSet};
use compiler::grammar::{instruction::Instruction, Capability};
use mail_parser::Message;
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

pub struct Runtime {
    pub(crate) allowed_capabilities: AHashSet<Capability>,
    pub(crate) environment: AHashMap<String, String>,
    pub(crate) include_scripts: AHashMap<String, Arc<Sieve>>,

    pub(crate) max_include_scripts: usize,
    pub(crate) max_instructions: usize,
}

#[derive(Clone)]
pub struct Context<'x> {
    pub(crate) runtime: &'x Runtime,
    pub(crate) envelope: Vec<(Envelope<'x>, Cow<'x, str>)>,
    pub(crate) part: usize,
    pub(crate) part_iter: IntoIter<usize>,
    pub(crate) part_iter_stack: Vec<(usize, IntoIter<usize>)>,

    pub(crate) pos: usize,
    pub(crate) test_result: bool,
    pub(crate) script_cache: AHashMap<Script, Arc<Sieve>>,
    pub(crate) script_stack: Vec<ScriptStack>,
    pub(crate) vars_global: AHashMap<String, String>,
    pub(crate) vars_local: Vec<String>,
    pub(crate) vars_match: Vec<String>,
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
    use std::fs;

    use mail_parser::Message;

    use crate::{Compiler, Event, Input, Runtime};

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
        let script = compiler
            .compile(&fs::read("tests/extensions/body/text.svtest").unwrap())
            .unwrap();
        //tests/test-header.svtest

        let runtime = Runtime::new();
        let mut instance = runtime.instance();
        let mut input = Input::script("", script);
        let mut raw_message = Vec::new();
        let mut current_test = String::new();

        'outer: loop {
            let message = Message::parse(&raw_message).unwrap_or_default();

            while let Some(event) = instance.run(&message, input) {
                match event.unwrap() {
                    Event::IncludeScript { name } => {
                        //include_script = compiler.compile(&fs::read(&name).unwrap()).unwrap().into();
                        //input = Input::Script(included_scripts.last().unwrap());
                        //input = Input::Script(include_script.as_ref().unwrap());
                        let script = compiler.compile(&fs::read(name.as_str()).unwrap()).unwrap();
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

                                    raw_message = params.pop().unwrap().into_bytes();
                                    continue 'outer;
                                } else if let Some(envelope) = target.strip_prefix("envelope.") {
                                    instance
                                        .set_envelope(envelope.to_string(), params.pop().unwrap());
                                } else {
                                    panic!("test_set {} not implemented.", target);
                                }
                            }
                            "test_fail" => {
                                panic!("Test '{}' failed: {}", current_test, params.pop().unwrap());
                            }
                            _ => panic!("Test command {} not implemented.", command),
                        }
                    }
                }
            }
            break;
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
