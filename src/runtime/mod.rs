use std::{borrow::Cow, fmt::Display, ops::Deref, sync::Arc};

use ahash::{AHashMap, AHashSet};
use mail_parser::HeaderName;
use phf::phf_map;

use crate::{
    compiler::grammar::{Capability, Comparator, Invalid},
    Context, Envelope, Input, Runtime, Script, Sieve,
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
            environment: AHashMap::new(),
            include_scripts: AHashMap::new(),
            max_include_scripts: 3,
            max_instructions: 5000,
            protected_headers: Vec::new(),
        }
    }

    pub fn add_protected_header(&mut self, header_name: impl Into<Cow<'static, str>>) {
        self.protected_headers.push(HeaderName::parse(header_name));
    }

    pub fn with_protected_header(mut self, header_name: impl Into<Cow<'static, str>>) -> Self {
        self.add_protected_header(header_name);
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

impl<'x> From<String> for Envelope<'x> {
    fn from(name: String) -> Self {
        if let Some(envelope) = ENVELOPE.get(&name) {
            envelope.clone()
        } else {
            Envelope::Other(name.into())
        }
    }
}

impl<'x> From<&'x str> for Envelope<'x> {
    fn from(name: &'x str) -> Self {
        if let Some(envelope) = ENVELOPE.get(name) {
            envelope.clone()
        } else {
            Envelope::Other(name.into())
        }
    }
}

pub(crate) static ENVELOPE: phf::Map<&'static str, Envelope> = phf_map! {
    "from" => Envelope::From,
    "to" => Envelope::To,
    "bytimeabsolute" => Envelope::ByTimeAbsolute,
    "bytimerelative" => Envelope::ByTimeRelative,
    "bymode" => Envelope::ByMode,
    "bytrace" => Envelope::ByTrace,
};
