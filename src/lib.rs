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

}

#[derive(Debug, Clone, Eq, PartialEq)]
pub enum Input {
    True,
    False,
    Script { name: Script, script: Arc<Sieve> },
}
