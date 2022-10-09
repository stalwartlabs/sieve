use std::{ops::Deref, sync::Arc};

use ahash::{AHashMap, AHashSet};
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
