use std::{borrow::Cow, fmt::Display, ops::Deref, sync::Arc};

use ahash::{AHashMap, AHashSet};
use mail_parser::HeaderName;

use crate::{
    compiler::grammar::{Capability, Comparator, Invalid},
    Context, Input, Metadata, Runtime, Script, Sieve,
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
        let allowed_capabilities = AHashSet::from_iter([
            Capability::Envelope,
            Capability::EnvelopeDsn,
            Capability::EnvelopeDeliverBy,
            Capability::FileInto,
            Capability::EncodedCharacter,
            Capability::Comparator(Comparator::Elbonia),
            Capability::Comparator(Comparator::AsciiCaseMap),
            Capability::Comparator(Comparator::AsciiNumeric),
            Capability::Comparator(Comparator::Octet),
            Capability::Body,
            Capability::Convert,
            Capability::Copy,
            Capability::Relational,
            Capability::Date,
            Capability::Index,
            Capability::Duplicate,
            Capability::Variables,
            Capability::EditHeader,
            Capability::ForEveryPart,
            Capability::Mime,
            Capability::Replace,
            Capability::Enclose,
            Capability::ExtractText,
            Capability::Enotify,
            Capability::RedirectDsn,
            Capability::RedirectDeliverBy,
            Capability::Environment,
            Capability::Reject,
            Capability::Ereject,
            Capability::ExtLists,
            Capability::SubAddress,
            Capability::Vacation,
            Capability::VacationSeconds,
            Capability::Fcc,
            Capability::Mailbox,
            Capability::MailboxId,
            Capability::MboxMetadata,
            Capability::ServerMetadata,
            Capability::SpecialUse,
            Capability::Imap4Flags,
            Capability::Ihave,
            Capability::ImapSieve,
            Capability::Include,
            Capability::Regex,
            Capability::SpamTest,
            Capability::SpamTestPlus,
            Capability::VirusTest,
            #[cfg(test)]
            Capability::Other("vnd.stalwart.testsuite".to_string()),
        ]);

        Runtime {
            allowed_capabilities,
            environment: AHashMap::from_iter([
                ("name".into(), "Stalwart Sieve".into()),
                ("version".into(), env!("CARGO_PKG_VERSION").into()),
            ]),
            metadata: Vec::new(),
            include_scripts: AHashMap::new(),
            max_include_scripts: 3,
            max_instructions: 5000,
            max_variable_size: 4096,
            max_redirects: 1,
            protected_headers: vec![
                HeaderName::Other("Original-Subject".into()),
                HeaderName::Other("Original-From".into()),
            ],
        }
    }

    pub fn set_max_variable_size(&mut self, size: usize) {
        self.max_variable_size = size;
    }

    pub fn with_max_variable_size(mut self, size: usize) -> Self {
        self.max_variable_size = size;
        self
    }

    pub fn set_protected_header(&mut self, header_name: impl Into<Cow<'static, str>>) {
        self.protected_headers.push(HeaderName::parse(header_name));
    }

    pub fn with_protected_header(mut self, header_name: impl Into<Cow<'static, str>>) -> Self {
        self.set_protected_header(header_name);
        self
    }

    pub fn set_env_variable(
        &mut self,
        name: impl Into<String>,
        value: impl Into<Cow<'static, str>>,
    ) {
        self.environment.insert(name.into(), value.into());
    }

    pub fn with_env_variable(
        mut self,
        name: impl Into<String>,
        value: impl Into<Cow<'static, str>>,
    ) -> Self {
        self.set_env_variable(name.into(), value.into());
        self
    }

    pub fn set_medatata(
        &mut self,
        name: impl Into<Metadata<String>>,
        value: impl Into<Cow<'static, str>>,
    ) {
        self.metadata.push((name.into(), value.into()));
    }

    pub fn with_metadata(
        mut self,
        name: impl Into<Metadata<String>>,
        value: impl Into<Cow<'static, str>>,
    ) -> Self {
        self.set_medatata(name, value);
        self
    }

    pub fn filter<'z: 'x, 'x>(&'z self, raw_message: &'x [u8]) -> Context<'x> {
        Context::new(self, raw_message)
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

impl From<bool> for Input {
    fn from(value: bool) -> Self {
        if value {
            Input::True
        } else {
            Input::False
        }
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

impl Display for Script {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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

impl<T> Metadata<T> {
    pub fn server(annotation: impl Into<T>) -> Self {
        Metadata::Server {
            annotation: annotation.into(),
        }
    }

    pub fn mailbox(name: impl Into<T>, annotation: impl Into<T>) -> Self {
        Metadata::Mailbox {
            name: name.into(),
            annotation: annotation.into(),
        }
    }
}

impl From<String> for Metadata<String> {
    fn from(annotation: String) -> Self {
        Metadata::Server { annotation }
    }
}

impl From<&'_ str> for Metadata<String> {
    fn from(annotation: &'_ str) -> Self {
        Metadata::Server {
            annotation: annotation.to_string(),
        }
    }
}

impl From<(String, String)> for Metadata<String> {
    fn from((name, annotation): (String, String)) -> Self {
        Metadata::Mailbox { name, annotation }
    }
}

impl From<(&'_ str, &'_ str)> for Metadata<String> {
    fn from((name, annotation): (&'_ str, &'_ str)) -> Self {
        Metadata::Mailbox {
            name: name.to_string(),
            annotation: annotation.to_string(),
        }
    }
}
