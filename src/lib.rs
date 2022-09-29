use compiler::grammar::{capability::Capability, command::Command};
use serde::{Deserialize, Serialize};

pub mod compiler;
pub mod runtime;

#[derive(Debug, Serialize, Deserialize)]
pub struct Sieve {
    capabilities: Vec<Capability>,
    commands: Vec<Command>,
}

pub struct Compiler {
    // Settings
    pub(crate) max_script_len: usize,
    pub(crate) max_string_len: usize,
    pub(crate) max_variable_len: usize,
    pub(crate) max_nested_blocks: usize,
    pub(crate) max_nested_tests: usize,
}
