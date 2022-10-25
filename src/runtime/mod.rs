/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use std::{borrow::Cow, fmt::Display, ops::Deref, sync::Arc};

use ahash::{AHashMap, AHashSet};
use mail_parser::{Encoding, HeaderName, Message, MessagePart, PartType};

use crate::{
    compiler::grammar::{Capability, Comparator, Invalid},
    Context, Input, Metadata, Runtime, Script, Sieve,
};

pub mod actions;
pub mod context;
pub mod serialize;
pub mod string;
pub mod tests;
pub mod variables;

#[derive(Debug)]
pub enum RuntimeError {
    TooManyIncludes,
    InvalidInstruction(Invalid),
    ScriptErrorMessage(String),
    CapabilityNotAllowed(Capability),
    CapabilityNotSupported(String),
    CPULimitReached,
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
            cpu_limit: 5000,
            max_variable_size: 4096,
            max_redirects: 1,
            max_received_headers: 10,
            protected_headers: vec![
                HeaderName::Other("Original-Subject".into()),
                HeaderName::Other("Original-From".into()),
            ],
            valid_notification_uris: AHashSet::new(),
            valid_ext_lists: AHashSet::new(),
            vacation_use_orig_rcpt: false,
            vacation_default_subject: "Automated reply".into(),
            vacation_subject_prefix: "Auto: ".into(),
            max_header_size: 1024,
            max_out_messages: 3,
            default_vacation_expiry: 30 * 86400,
            default_duplicate_expiry: 7 * 86400,
        }
    }

    pub fn set_cpu_limit(&mut self, size: usize) {
        self.cpu_limit = size;
    }

    pub fn with_cpu_limit(mut self, size: usize) -> Self {
        self.cpu_limit = size;
        self
    }

    pub fn set_max_include_scripts(&mut self, size: usize) {
        self.max_include_scripts = size;
    }

    pub fn with_max_include_scripts(mut self, size: usize) -> Self {
        self.max_include_scripts = size;
        self
    }

    pub fn set_max_redirects(&mut self, size: usize) {
        self.max_redirects = size;
    }

    pub fn with_max_redirects(mut self, size: usize) -> Self {
        self.max_redirects = size;
        self
    }

    pub fn set_max_out_messages(&mut self, size: usize) {
        self.max_out_messages = size;
    }

    pub fn with_max_out_messages(mut self, size: usize) -> Self {
        self.max_out_messages = size;
        self
    }

    pub fn set_max_received_headers(&mut self, size: usize) {
        self.max_received_headers = size;
    }

    pub fn with_max_received_headers(mut self, size: usize) -> Self {
        self.max_received_headers = size;
        self
    }

    pub fn set_max_variable_size(&mut self, size: usize) {
        self.max_variable_size = size;
    }

    pub fn with_max_variable_size(mut self, size: usize) -> Self {
        self.max_variable_size = size;
        self
    }

    pub fn set_max_header_size(&mut self, size: usize) {
        self.max_header_size = size;
    }

    pub fn with_max_header_size(mut self, size: usize) -> Self {
        self.max_header_size = size;
        self
    }

    pub fn set_default_vacation_expiry(&mut self, expiry: u64) {
        self.default_vacation_expiry = expiry;
    }

    pub fn with_default_vacation_expiry(mut self, expiry: u64) -> Self {
        self.default_vacation_expiry = expiry;
        self
    }

    pub fn set_default_duplicate_expiry(&mut self, expiry: u64) {
        self.default_duplicate_expiry = expiry;
    }

    pub fn with_default_duplicate_expiry(mut self, expiry: u64) -> Self {
        self.default_duplicate_expiry = expiry;
        self
    }

    pub fn set_capability(&mut self, capability: impl Into<Capability>) {
        self.allowed_capabilities.insert(capability.into());
    }

    pub fn with_capability(mut self, capability: impl Into<Capability>) -> Self {
        self.set_capability(capability);
        self
    }

    pub fn unset_capability(&mut self, capability: impl Into<Capability>) {
        self.allowed_capabilities.remove(&capability.into());
    }

    pub fn without_capability(mut self, capability: impl Into<Capability>) -> Self {
        self.unset_capability(capability);
        self
    }

    pub fn set_protected_header(&mut self, header_name: impl Into<Cow<'static, str>>) {
        if let Some(header_name) = HeaderName::parse(header_name) {
            self.protected_headers.push(header_name);
        }
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

    pub fn set_valid_notification_uri(&mut self, uri: impl Into<Cow<'static, str>>) {
        self.valid_notification_uris.insert(uri.into());
    }

    pub fn with_valid_notification_uri(mut self, uri: impl Into<Cow<'static, str>>) -> Self {
        self.valid_notification_uris.insert(uri.into());
        self
    }

    pub fn set_valid_ext_list(&mut self, name: impl Into<Cow<'static, str>>) {
        self.valid_ext_lists.insert(name.into());
    }

    pub fn with_valid_ext_list(mut self, name: impl Into<Cow<'static, str>>) -> Self {
        self.set_valid_ext_list(name);
        self
    }

    pub fn set_vacation_use_orig_rcpt(&mut self, value: bool) {
        self.vacation_use_orig_rcpt = value;
    }

    pub fn with_vacation_use_orig_rcpt(mut self, value: bool) -> Self {
        self.set_vacation_use_orig_rcpt(value);
        self
    }

    pub fn set_vacation_default_subject(&mut self, value: impl Into<Cow<'static, str>>) {
        self.vacation_default_subject = value.into();
    }

    pub fn with_vacation_default_subject(mut self, value: impl Into<Cow<'static, str>>) -> Self {
        self.set_vacation_default_subject(value);
        self
    }

    pub fn set_vacation_subject_prefix(&mut self, value: impl Into<Cow<'static, str>>) {
        self.vacation_subject_prefix = value.into();
    }

    pub fn with_vacation_subject_prefix(mut self, value: impl Into<Cow<'static, str>>) -> Self {
        self.set_vacation_subject_prefix(value);
        self
    }

    pub fn filter<'z: 'x, 'x>(&'z self, raw_message: &'x [u8]) -> Context<'x> {
        Context::new(
            self,
            Message::parse(raw_message).unwrap_or_else(|| Message {
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
                ..Default::default()
            }),
        )
    }

    pub fn filter_parsed<'z: 'x, 'x>(&'z self, message: Message<'x>) -> Context<'x> {
        Context::new(self, message)
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
