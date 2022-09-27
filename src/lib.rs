use runtime::Command;

pub mod compiler;
pub mod runtime;

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

#[derive(Debug, Clone)]
pub enum Capability {
    Envelope,
    FileInto,
    EncodedCharacter,
    Comparator(Comparator),
    Other(String),
}

#[derive(Debug, Clone)]
pub enum Comparator {
    Elbonia,
    Octet,
    AsciiCaseMap,
    AsciiNumeric,
    Other(String),
}
