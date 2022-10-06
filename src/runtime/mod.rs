use std::{ops::Deref, sync::Arc};

use ahash::{AHashMap, AHashSet};

use crate::{
    compiler::grammar::{Capability, Invalid},
    Context, Input, Runtime, Script, Sieve,
};

pub mod actions;
pub mod context;
pub mod string;
pub mod tests;
pub mod variables;

#[derive(Debug)]
pub enum RuntimeError {
    IllegalAction,
    TooManyIncludes,
    InvalidInstruction(Invalid),
    ScriptErrorMessage(String),
    CapabilityNotAllowed(Capability),
    CapabilityNotSupported(String),
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            allowed_capabilities: AHashSet::new(),
            environment: AHashMap::new(),
            include_scripts: AHashMap::new(),
            max_include_scripts: 3,
            max_instructions: 5000,
        }
    }

    pub fn instance(&self) -> Context {
        Context::new(self)
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}

impl Input {
    pub fn script(name: impl Into<Script>, script: impl Into<Arc<Sieve>>) -> Self {
        Input::Script {
            name: name.into(),
            script: script.into(),
        }
    }

    pub fn success() -> Self {
        Input::True
    }

    pub fn fail() -> Self {
        Input::False
    }
}

impl Deref for Script {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        match self {
            Script::Personal(name) | Script::Global(name) => name,
        }
    }
}

impl AsRef<str> for Script {
    fn as_ref(&self) -> &str {
        match self {
            Script::Personal(name) | Script::Global(name) => name.as_str(),
        }
    }
}

impl AsRef<String> for Script {
    fn as_ref(&self) -> &String {
        match self {
            Script::Personal(name) | Script::Global(name) => name,
        }
    }
}

impl Script {
    pub(crate) fn as_str(&self) -> &String {
        match self {
            Script::Personal(name) | Script::Global(name) => name,
        }
    }
}

impl From<String> for Script {
    fn from(name: String) -> Self {
        Script::Personal(name)
    }
}

impl From<&str> for Script {
    fn from(name: &str) -> Self {
        Script::Personal(name.to_string())
    }
}
