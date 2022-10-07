use std::{sync::Arc, vec::IntoIter};

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

pub struct Context<'x, 'y> {
    pub(crate) runtime: &'y Runtime,
    pub(crate) raw_message: &'x [u8],
    pub(crate) message: Option<Message<'x>>,
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

    #[cfg(test)]
    pub(crate) test_name: String,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub enum Script {
    Personal(String),
    Global(String),
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Event {
    IncludeScript { name: Script },
    MailboxExists { names: Vec<String> },
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
        collections::{BTreeSet, HashSet},
        fs,
        path::PathBuf,
        pin::Pin,
        sync::Arc,
    };

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
        let script = compiler
            .compile(&fs::read("tests/lexer.svtest").unwrap())
            .unwrap();

        let runtime = Runtime::new();
        let mut instance = runtime.instance();
        let mut input = Input::script("", script);

        while let Some(event) = instance.run(input) {
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
            }
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
