/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
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

pub mod actions;
pub mod context;
pub mod eval;
pub mod expression;
pub mod serialize;
pub mod tests;
pub mod variables;

use std::{borrow::Cow, fmt::Display, ops::Deref, sync::Arc};

use ahash::{AHashMap, AHashSet};
use mail_parser::{Encoding, HeaderName, Message, MessageParser, MessagePart, PartType};

use crate::{
    compiler::{
        grammar::{Capability, Invalid},
        Number, Regex, VariableType,
    },
    Context, Function, FunctionMap, Input, Metadata, PluginArgument, Runtime, Script, SetVariable,
    Sieve,
};

use self::eval::ToString;

#[derive(Debug, Clone)]
pub enum Variable<'x> {
    String(String),
    StringRef(&'x str),
    Integer(i64),
    Float(f64),
    Array(Vec<Variable<'x>>),
    ArrayRef(&'x Vec<Variable<'x>>),
}

#[derive(Debug)]
pub enum RuntimeError {
    TooManyIncludes,
    InvalidInstruction(Invalid),
    ScriptErrorMessage(String),
    CapabilityNotAllowed(Capability),
    CapabilityNotSupported(String),
    CPULimitReached,
}

impl<'x> Default for Variable<'x> {
    fn default() -> Self {
        Variable::StringRef("")
    }
}

impl<'x> Variable<'x> {
    pub fn into_cow(self) -> Cow<'x, str> {
        match self {
            Variable::String(s) => Cow::Owned(s),
            Variable::StringRef(s) => Cow::Borrowed(s),
            Variable::Integer(n) => Cow::Owned(n.to_string()),
            Variable::Float(n) => Cow::Owned(n.to_string()),
            Variable::Array(l) => Cow::Owned(l.to_string()),
            Variable::ArrayRef(l) => Cow::Owned(l.to_string()),
        }
    }

    pub fn to_cow<'y: 'x>(&'y self) -> Cow<'x, str> {
        match self {
            Variable::String(s) => Cow::Borrowed(s.as_str()),
            Variable::StringRef(s) => Cow::Borrowed(*s),
            Variable::Integer(n) => Cow::Owned(n.to_string()),
            Variable::Float(n) => Cow::Owned(n.to_string()),
            Variable::Array(l) => Cow::Owned(l.to_string()),
            Variable::ArrayRef(l) => Cow::Owned(l.to_string()),
        }
    }

    pub fn into_string(self) -> String {
        match self {
            Variable::String(s) => s,
            Variable::StringRef(s) => s.to_string(),
            Variable::Integer(n) => n.to_string(),
            Variable::Float(n) => n.to_string(),
            Variable::Array(l) => l.to_string(),
            Variable::ArrayRef(l) => l.to_string(),
        }
    }

    pub fn to_number(&self) -> Number {
        self.to_number_checked()
            .unwrap_or(Number::Float(f64::INFINITY))
    }

    pub fn to_number_checked(&self) -> Option<Number> {
        let s = match self {
            Variable::Integer(n) => return Number::Integer(*n).into(),
            Variable::Float(n) => return Number::Float(*n).into(),
            Variable::String(s) if !s.is_empty() => s.as_str(),
            Variable::StringRef(s) if !s.is_empty() => *s,
            _ => return None,
        };

        if !s.contains('.') {
            s.parse::<i64>().map(Number::Integer).ok()
        } else {
            s.parse::<f64>().map(Number::Float).ok()
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Variable::String(s) => s.len(),
            Variable::StringRef(s) => s.len(),
            Variable::Integer(_) | Variable::Float(_) => 2,
            Variable::Array(l) => l.iter().map(|v| v.len() + 2).sum(),
            Variable::ArrayRef(l) => l.iter().map(|v| v.len() + 2).sum(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Variable::String(s) => s.is_empty(),
            Variable::StringRef(s) => s.is_empty(),
            _ => false,
        }
    }

    pub fn to_owned(&self) -> Variable<'static> {
        match self {
            Variable::String(s) => Variable::String(s.to_string()),
            Variable::StringRef(s) => Variable::String(s.to_string()),
            Variable::Integer(n) => Variable::Integer(*n),
            Variable::Float(n) => Variable::Float(*n),
            Variable::Array(l) => Variable::Array(l.iter().map(Variable::to_owned).collect()),
            Variable::ArrayRef(l) => Variable::Array(l.iter().map(Variable::to_owned).collect()),
        }
    }

    pub fn into_owned(self) -> Variable<'static> {
        match self {
            Variable::String(s) => Variable::String(s),
            Variable::StringRef(s) => Variable::String(s.to_string()),
            Variable::Integer(n) => Variable::Integer(n),
            Variable::Float(n) => Variable::Float(n),
            Variable::Array(l) => {
                Variable::Array(l.into_iter().map(Variable::into_owned).collect())
            }
            Variable::ArrayRef(l) => Variable::Array(l.iter().map(Variable::to_owned).collect()),
        }
    }

    pub fn as_ref<'y: 'x>(&'y self) -> Variable<'x> {
        match self {
            Variable::String(s) => Variable::StringRef(s.as_str()),
            Variable::StringRef(s) => Variable::StringRef(s),
            Variable::Integer(n) => Variable::Integer(*n),
            Variable::Float(n) => Variable::Float(*n),
            Variable::Array(l) => Variable::ArrayRef(l),
            Variable::ArrayRef(l) => Variable::ArrayRef(l),
        }
    }
}

impl<'x> From<&'x str> for Variable<'x> {
    fn from(s: &'x str) -> Self {
        Variable::StringRef(s)
    }
}

impl From<String> for Variable<'_> {
    fn from(s: String) -> Self {
        Variable::String(s)
    }
}

impl<'x> From<&'x String> for Variable<'x> {
    fn from(s: &'x String) -> Self {
        Variable::StringRef(s.as_str())
    }
}

impl<'x> From<Cow<'x, str>> for Variable<'x> {
    fn from(s: Cow<'x, str>) -> Self {
        match s {
            Cow::Borrowed(s) => Variable::StringRef(s),
            Cow::Owned(s) => Variable::String(s),
        }
    }
}

impl From<Number> for Variable<'_> {
    fn from(n: Number) -> Self {
        match n {
            Number::Integer(n) => Variable::Integer(n),
            Number::Float(n) => Variable::Float(n),
        }
    }
}

impl From<usize> for Variable<'_> {
    fn from(n: usize) -> Self {
        Variable::Integer(n as i64)
    }
}

impl From<i64> for Variable<'_> {
    fn from(n: i64) -> Self {
        Variable::Integer(n)
    }
}

impl From<u64> for Variable<'_> {
    fn from(n: u64) -> Self {
        Variable::Integer(n as i64)
    }
}

impl From<f64> for Variable<'_> {
    fn from(n: f64) -> Self {
        Variable::Float(n)
    }
}

impl From<i32> for Variable<'_> {
    fn from(n: i32) -> Self {
        Variable::Integer(n as i64)
    }
}

impl From<bool> for Variable<'_> {
    fn from(b: bool) -> Self {
        Variable::Integer(i64::from(b))
    }
}

impl PartialEq for Number {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Float(b)) => (*a as f64) == *b,
            (Self::Float(a), Self::Integer(b)) => *a == (*b as f64),
        }
    }
}

impl Eq for Number {}

impl PartialOrd for Number {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        let (a, b) = match (self, other) {
            (Number::Integer(a), Number::Integer(b)) => return a.partial_cmp(b),
            (Number::Float(a), Number::Float(b)) => (*a, *b),
            (Number::Integer(a), Number::Float(b)) => (*a as f64, *b),
            (Number::Float(a), Number::Integer(b)) => (*a, *b as f64),
        };
        a.partial_cmp(&b)
    }
}

impl<'x> self::eval::ToString for Vec<Variable<'x>> {
    fn to_string(&self) -> String {
        let mut result = String::with_capacity(self.len() * 10);
        for item in self {
            if !result.is_empty() {
                result.push_str("\r\n");
            }
            match item {
                Variable::String(v) => result.push_str(v),
                Variable::StringRef(v) => result.push_str(v),
                Variable::Integer(v) => result.push_str(&v.to_string()),
                Variable::Float(v) => result.push_str(&v.to_string()),
                Variable::Array(_) | Variable::ArrayRef(_) => {}
            }
        }
        result
    }
}

impl PluginArgument<String, Number> {
    pub fn unwrap_string(self) -> Option<String> {
        match self {
            PluginArgument::Text(s) => s.into(),
            PluginArgument::Number(n) => n.to_string().into(),
            _ => None,
        }
    }

    pub fn unwrap_number(self) -> Option<Number> {
        match self {
            PluginArgument::Number(n) => n.into(),
            _ => None,
        }
    }

    pub fn unwrap_regex(self) -> Option<Regex> {
        match self {
            PluginArgument::Regex(r) => r.into(),
            _ => None,
        }
    }

    pub fn unwrap_array(self) -> Option<Vec<Self>> {
        match self {
            PluginArgument::Array(a) => a.into(),
            _ => None,
        }
    }

    pub fn unwrap_variable(self) -> Option<VariableType> {
        match self {
            PluginArgument::Variable(v) => v.into(),
            _ => None,
        }
    }

    pub fn unwrap_string_array(self) -> Option<Vec<String>> {
        match self {
            PluginArgument::Array(a) => a
                .into_iter()
                .filter_map(Self::unwrap_string)
                .collect::<Vec<_>>()
                .into(),
            _ => None,
        }
    }

    pub fn unwrap_number_array(self) -> Option<Vec<Number>> {
        match self {
            PluginArgument::Array(a) => a
                .into_iter()
                .filter_map(Self::unwrap_number)
                .collect::<Vec<_>>()
                .into(),
            _ => None,
        }
    }

    pub fn unwrap_regex_array(self) -> Option<Vec<Regex>> {
        match self {
            PluginArgument::Array(a) => a
                .into_iter()
                .filter_map(Self::unwrap_regex)
                .collect::<Vec<_>>()
                .into(),
            _ => None,
        }
    }

    pub fn unwrap_variable_array(self) -> Option<Vec<VariableType>> {
        match self {
            PluginArgument::Array(a) => a
                .into_iter()
                .filter_map(Self::unwrap_variable)
                .collect::<Vec<_>>()
                .into(),
            _ => None,
        }
    }
}

impl Runtime {
    pub fn new() -> Self {
        #[allow(unused_mut)]
        let mut allowed_capabilities = AHashSet::from_iter(Capability::all().iter().cloned());

        #[cfg(test)]
        allowed_capabilities.insert(Capability::Other("vnd.stalwart.testsuite".to_string()));

        Runtime {
            allowed_capabilities,
            environment: AHashMap::from_iter([
                ("name".into(), "Stalwart Sieve".into()),
                ("version".into(), env!("CARGO_PKG_VERSION").into()),
            ]),
            metadata: Vec::new(),
            include_scripts: AHashMap::new(),
            max_nested_includes: 3,
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
            local_hostname: "localhost".into(),
            functions: Vec::new(),
        }
    }

    pub fn set_cpu_limit(&mut self, size: usize) {
        self.cpu_limit = size;
    }

    pub fn with_cpu_limit(mut self, size: usize) -> Self {
        self.cpu_limit = size;
        self
    }

    pub fn set_max_nested_includes(&mut self, size: usize) {
        self.max_nested_includes = size;
    }

    pub fn with_max_nested_includes(mut self, size: usize) -> Self {
        self.max_nested_includes = size;
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

    pub fn without_capabilities(
        mut self,
        capabilities: impl IntoIterator<Item = impl Into<Capability>>,
    ) -> Self {
        for capability in capabilities {
            self.allowed_capabilities.remove(&capability.into());
        }
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

    pub fn with_protected_headers(
        mut self,
        header_names: impl IntoIterator<Item = impl Into<Cow<'static, str>>>,
    ) -> Self {
        self.protected_headers = header_names
            .into_iter()
            .filter_map(HeaderName::parse)
            .collect();
        self
    }

    pub fn set_env_variable(
        &mut self,
        name: impl Into<Cow<'static, str>>,
        value: impl Into<Variable<'static>>,
    ) {
        self.environment.insert(name.into(), value.into());
    }

    pub fn with_env_variable(
        mut self,
        name: impl Into<Cow<'static, str>>,
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

    pub fn with_valid_notification_uris(
        mut self,
        uris: impl IntoIterator<Item = impl Into<Cow<'static, str>>>,
    ) -> Self {
        self.valid_notification_uris = uris.into_iter().map(Into::into).collect();
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

    pub fn with_valid_ext_lists(
        mut self,
        lists: impl IntoIterator<Item = impl Into<Cow<'static, str>>>,
    ) -> Self {
        self.valid_ext_lists = lists.into_iter().map(Into::into).collect();
        self
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

    pub fn set_local_hostname(&mut self, value: impl Into<Cow<'static, str>>) {
        self.local_hostname = value.into();
    }

    pub fn with_local_hostname(mut self, value: impl Into<Cow<'static, str>>) -> Self {
        self.set_local_hostname(value);
        self
    }

    pub fn with_functions(mut self, fnc_map: &mut FunctionMap) -> Self {
        self.functions = std::mem::take(&mut fnc_map.functions);
        self
    }

    pub fn set_functions(&mut self, fnc_map: &mut FunctionMap) {
        self.functions = std::mem::take(&mut fnc_map.functions);
    }

    pub fn filter<'z: 'x, 'x>(&'z self, raw_message: &'x [u8]) -> Context<'x> {
        Context::new(
            self,
            MessageParser::new()
                .parse(raw_message)
                .unwrap_or_else(|| Message {
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

impl FunctionMap {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_function(mut self, name: impl Into<String>, fnc: Function) -> Self {
        self.map
            .insert(name.into(), (self.functions.len() as u32, 1));
        self.functions.push(fnc);
        self
    }

    pub fn with_function_args(
        mut self,
        name: impl Into<String>,
        fnc: Function,
        num_args: u32,
    ) -> Self {
        self.map
            .insert(name.into(), (self.functions.len() as u32, num_args));
        self.functions.push(fnc);
        self
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

    pub fn variables(list: Vec<SetVariable>) -> Self {
        Input::Variables { list }
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
    pub fn into_string(self) -> String {
        match self {
            Script::Personal(name) | Script::Global(name) => name,
        }
    }

    pub fn as_str(&self) -> &String {
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
