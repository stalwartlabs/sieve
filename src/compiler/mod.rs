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

use std::{borrow::Cow, fmt::Display};

use ahash::AHashMap;
use mail_parser::HeaderName;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::{
    runtime::RuntimeError, Compiler, Envelope, ExternalId, FunctionMap, PluginSchema,
    PluginSchemaArgument, PluginSchemaTag,
};

use self::{
    grammar::{expr::Expression, AddressPart, Capability},
    lexer::tokenizer::TokenInfo,
};

pub mod grammar;
pub mod lexer;

#[derive(Debug)]
pub struct CompileError {
    line_num: usize,
    line_pos: usize,
    error_type: ErrorType,
}

#[derive(Debug)]
pub enum ErrorType {
    InvalidCharacter(u8),
    InvalidNumber(String),
    InvalidMatchVariable(usize),
    InvalidUnicodeSequence(u32),
    InvalidNamespace(String),
    InvalidRegex(String),
    InvalidExpression(String),
    InvalidUtf8String,
    InvalidHeaderName,
    InvalidArguments,
    InvalidAddress,
    InvalidURI,
    InvalidEnvelope(String),
    UnterminatedString,
    UnterminatedComment,
    UnterminatedMultiline,
    UnterminatedBlock,
    ScriptTooLong,
    StringTooLong,
    VariableTooLong,
    VariableIsLocal(String),
    HeaderTooLong,
    ExpectedConstantString,
    UnexpectedToken {
        expected: Cow<'static, str>,
        found: String,
    },
    UnexpectedEOF,
    TooManyNestedBlocks,
    TooManyNestedTests,
    TooManyNestedForEveryParts,
    TooManyIncludes,
    LabelAlreadyDefined(String),
    LabelUndefined(String),
    BreakOutsideLoop,
    UnsupportedComparator(String),
    DuplicatedParameter,
    UndeclaredCapability(Capability),
    MissingTag(Cow<'static, str>),
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Value {
    Text(String),
    Number(Number),
    Variable(VariableType),
    Expression(Vec<Expression>),
    Regex(Regex),
    List(Vec<Value>),
}

#[derive(Debug, Clone)]
pub struct Regex {
    pub regex: fancy_regex::Regex,
    pub expr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum VariableType {
    Local(usize),
    Match(usize),
    Global(String),
    Environment(String),
    Envelope(Envelope),
    Header(HeaderVariable),
    Part(MessagePart),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Transform {
    pub variable: Box<VariableType>,
    pub functions: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderVariable {
    pub name: Vec<HeaderName<'static>>,
    pub part: HeaderPart,
    pub index_hdr: i32,
    pub index_part: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MessagePart {
    TextBody(bool),
    HtmlBody(bool),
    Contents,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum HeaderPart {
    Text,
    Date,
    Id,
    Address(AddressPart),
    ContentType(ContentTypePart),
    Received(ReceivedPart),
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContentTypePart {
    Type,
    Subtype,
    Attribute(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceivedPart {
    From(ReceivedHostname),
    FromIp,
    FromIpRev,
    By(ReceivedHostname),
    For,
    With,
    TlsVersion,
    TlsCipher,
    Id,
    Ident,
    Via,
    Date,
    DateRaw,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReceivedHostname {
    Name,
    Ip,
    Any,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Number {
    Integer(i64),
    Float(f64),
}

impl Number {
    #[cfg(test)]
    pub fn to_float(&self) -> f64 {
        match self {
            Number::Integer(i) => *i as f64,
            Number::Float(fl) => *fl,
        }
    }
}

impl From<Number> for usize {
    fn from(value: Number) -> Self {
        match value {
            Number::Integer(i) => i as usize,
            Number::Float(fl) => fl as usize,
        }
    }
}

impl Display for Number {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Number::Integer(i) => i.fmt(f),
            Number::Float(fl) => fl.fmt(f),
        }
    }
}

impl Compiler {
    pub const VERSION: u32 = 2;

    pub fn new() -> Self {
        Compiler {
            max_script_size: 1024 * 1024,
            max_string_size: 4096,
            max_variable_name_size: 32,
            max_nested_blocks: 15,
            max_nested_tests: 15,
            max_nested_foreverypart: 3,
            max_match_variables: 30,
            max_local_variables: 128,
            max_header_size: 1024,
            max_includes: 6,
            plugins: AHashMap::new(),
            functions: AHashMap::new(),
            no_capability_check: false,
        }
    }

    pub fn set_max_header_size(&mut self, size: usize) {
        self.max_header_size = size;
    }

    pub fn with_max_header_size(mut self, size: usize) -> Self {
        self.max_header_size = size;
        self
    }

    pub fn set_max_includes(&mut self, size: usize) {
        self.max_includes = size;
    }

    pub fn with_max_includes(mut self, size: usize) -> Self {
        self.max_includes = size;
        self
    }

    pub fn set_max_nested_blocks(&mut self, size: usize) {
        self.max_nested_blocks = size;
    }

    pub fn with_max_nested_blocks(mut self, size: usize) -> Self {
        self.max_nested_blocks = size;
        self
    }

    pub fn set_max_nested_tests(&mut self, size: usize) {
        self.max_nested_tests = size;
    }

    pub fn with_max_nested_tests(mut self, size: usize) -> Self {
        self.max_nested_tests = size;
        self
    }

    pub fn set_max_nested_foreverypart(&mut self, size: usize) {
        self.max_nested_foreverypart = size;
    }

    pub fn with_max_nested_foreverypart(mut self, size: usize) -> Self {
        self.max_nested_foreverypart = size;
        self
    }

    pub fn set_max_script_size(&mut self, size: usize) {
        self.max_script_size = size;
    }

    pub fn with_max_script_size(mut self, size: usize) -> Self {
        self.max_script_size = size;
        self
    }

    pub fn set_max_string_size(&mut self, size: usize) {
        self.max_string_size = size;
    }

    pub fn with_max_string_size(mut self, size: usize) -> Self {
        self.max_string_size = size;
        self
    }

    pub fn set_max_variable_name_size(&mut self, size: usize) {
        self.max_variable_name_size = size;
    }

    pub fn with_max_variable_name_size(mut self, size: usize) -> Self {
        self.max_variable_name_size = size;
        self
    }

    pub fn set_max_match_variables(&mut self, size: usize) {
        self.max_match_variables = size;
    }

    pub fn with_max_match_variables(mut self, size: usize) -> Self {
        self.max_match_variables = size;
        self
    }

    pub fn set_max_local_variables(&mut self, size: usize) {
        self.max_local_variables = size;
    }

    pub fn with_max_local_variables(mut self, size: usize) -> Self {
        self.max_local_variables = size;
        self
    }

    pub fn register_plugin(&mut self, name: impl Into<String>) -> &mut PluginSchema {
        let id = self.plugins.len() as ExternalId;
        self.plugins
            .entry(name.into())
            .or_insert_with(|| PluginSchema {
                id,
                tags: AHashMap::new(),
                arguments: Vec::new(),
            })
    }

    pub fn register_functions(mut self, fnc_map: &mut FunctionMap) -> Self {
        self.functions = std::mem::take(&mut fnc_map.map);
        self
    }

    pub fn with_no_capability_check(mut self, value: bool) -> Self {
        self.no_capability_check = value;
        self
    }

    pub fn set_no_capability_check(&mut self, value: bool) {
        self.no_capability_check = value;
    }
}

impl PluginSchema {
    pub fn with_id(&mut self, id: ExternalId) -> &mut Self {
        self.id = id;
        self
    }

    pub fn with_string_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Text);
        self
    }

    pub fn with_number_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Number);
        self
    }

    pub fn with_regex_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Regex);
        self
    }

    pub fn with_variable_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Variable);
        self
    }

    pub fn with_string_array_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Array(Box::new(
            PluginSchemaArgument::Text,
        )));
        self
    }

    pub fn with_number_array_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Array(Box::new(
            PluginSchemaArgument::Number,
        )));
        self
    }

    pub fn with_regex_array_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Array(Box::new(
            PluginSchemaArgument::Regex,
        )));
        self
    }

    pub fn with_variable_array_argument(&mut self) -> &mut Self {
        self.arguments.push(PluginSchemaArgument::Array(Box::new(
            PluginSchemaArgument::Variable,
        )));
        self
    }

    pub fn with_argument(&mut self, argument: PluginSchemaArgument) -> &mut Self {
        self.arguments.push(argument);
        self
    }

    pub fn with_tagged_argument(
        &mut self,
        tag: impl Into<String>,
        argument: PluginSchemaArgument,
    ) -> &mut Self {
        let id = self.tags.len() as ExternalId;
        self.tags.insert(
            tag.into(),
            PluginSchemaTag {
                id,
                argument: argument.into(),
            },
        );
        self
    }

    pub fn with_tagged_string_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(tag, PluginSchemaArgument::Text)
    }

    pub fn with_tagged_number_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(tag, PluginSchemaArgument::Number)
    }

    pub fn with_tagged_regex_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(tag, PluginSchemaArgument::Regex)
    }

    pub fn with_tagged_variable_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(tag, PluginSchemaArgument::Variable)
    }

    pub fn with_tagged_string_array_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(
            tag,
            PluginSchemaArgument::Array(Box::new(PluginSchemaArgument::Text)),
        )
    }

    pub fn with_tagged_number_array_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(
            tag,
            PluginSchemaArgument::Array(Box::new(PluginSchemaArgument::Number)),
        )
    }

    pub fn with_tagged_regex_array_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(
            tag,
            PluginSchemaArgument::Array(Box::new(PluginSchemaArgument::Regex)),
        )
    }

    pub fn with_tagged_variable_array_argument(&mut self, tag: impl Into<String>) -> &mut Self {
        self.with_tagged_argument(
            tag,
            PluginSchemaArgument::Array(Box::new(PluginSchemaArgument::Variable)),
        )
    }

    pub fn with_tag(&mut self, tag: impl Into<String>) -> &mut Self {
        let id = self.tags.len() as ExternalId;
        self.tags
            .insert(tag.into(), PluginSchemaTag { id, argument: None });
        self
    }
}

impl CompileError {
    pub fn line_num(&self) -> usize {
        self.line_num
    }

    pub fn line_pos(&self) -> usize {
        self.line_pos
    }

    pub fn error_type(&self) -> &ErrorType {
        &self.error_type
    }
}

impl PartialEq for Regex {
    fn eq(&self, other: &Self) -> bool {
        self.expr == other.expr
    }
}

impl Eq for Regex {}

impl Serialize for Regex {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.expr.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Regex {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        <String>::deserialize(deserializer).and_then(|expr| {
            fancy_regex::Regex::new(&expr)
                .map(|regex| Regex { regex, expr })
                .map_err(|err| serde::de::Error::custom(err.to_string()))
        })
    }
}

impl TokenInfo {
    pub fn expected(self, expected: impl Into<Cow<'static, str>>) -> CompileError {
        CompileError {
            line_num: self.line_num,
            line_pos: self.line_pos,
            error_type: ErrorType::UnexpectedToken {
                expected: expected.into(),
                found: self.token.to_string(),
            },
        }
    }

    pub fn missing_tag(self, tag: impl Into<Cow<'static, str>>) -> CompileError {
        CompileError {
            line_num: self.line_num,
            line_pos: self.line_pos,
            error_type: ErrorType::MissingTag(tag.into()),
        }
    }

    pub fn custom(self, error_type: ErrorType) -> CompileError {
        CompileError {
            line_num: self.line_num,
            line_pos: self.line_pos,
            error_type,
        }
    }
}

impl Display for CompileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.error_type() {
            ErrorType::InvalidCharacter(value) => {
                write!(f, "Invalid character {:?}", char::from(*value))
            }
            ErrorType::InvalidNumber(value) => write!(f, "Invalid number {value:?}"),
            ErrorType::InvalidMatchVariable(value) => {
                write!(f, "Match variable {value} out of range")
            }
            ErrorType::InvalidUnicodeSequence(value) => {
                write!(f, "Invalid Unicode sequence {value:04x}")
            }
            ErrorType::InvalidNamespace(value) => write!(f, "Invalid namespace {value:?}"),
            ErrorType::InvalidRegex(value) => write!(f, "Invalid regular expression {value:?}"),
            ErrorType::InvalidExpression(value) => write!(f, "Invalid expression {value}"),
            ErrorType::InvalidUtf8String => write!(f, "Invalid UTF-8 string"),
            ErrorType::InvalidHeaderName => write!(f, "Invalid header name"),
            ErrorType::InvalidArguments => write!(f, "Invalid Arguments"),
            ErrorType::InvalidAddress => write!(f, "Invalid Address"),
            ErrorType::InvalidURI => write!(f, "Invalid URI"),
            ErrorType::InvalidEnvelope(value) => write!(f, "Invalid envelope {value:?}"),
            ErrorType::UnterminatedString => write!(f, "Unterminated string"),
            ErrorType::UnterminatedComment => write!(f, "Unterminated comment"),
            ErrorType::UnterminatedMultiline => write!(f, "Unterminated multi-line string"),
            ErrorType::UnterminatedBlock => write!(f, "Unterminated block"),
            ErrorType::ScriptTooLong => write!(f, "Sieve script is too large"),
            ErrorType::StringTooLong => write!(f, "String is too long"),
            ErrorType::VariableTooLong => write!(f, "Variable name is too long"),
            ErrorType::VariableIsLocal(value) => {
                write!(f, "Variable {value:?} was already defined as local")
            }
            ErrorType::HeaderTooLong => write!(f, "Header value is too long"),
            ErrorType::ExpectedConstantString => write!(f, "Expected a constant string"),
            ErrorType::UnexpectedToken { expected, found } => {
                write!(f, "Expected token {expected:?} but found {found:?}")
            }
            ErrorType::UnexpectedEOF => write!(f, "Unexpected end of file"),
            ErrorType::TooManyNestedBlocks => write!(f, "Too many nested blocks"),
            ErrorType::TooManyNestedTests => write!(f, "Too many nested tests"),
            ErrorType::TooManyNestedForEveryParts => {
                write!(f, "Too many nested foreverypart blocks")
            }
            ErrorType::TooManyIncludes => write!(f, "Too many includes"),
            ErrorType::LabelAlreadyDefined(value) => write!(f, "Label {value:?} already defined"),
            ErrorType::LabelUndefined(value) => write!(f, "Label {value:?} does not exist"),
            ErrorType::BreakOutsideLoop => write!(f, "Break used outside of foreverypart loop"),
            ErrorType::UnsupportedComparator(value) => {
                write!(f, "Comparator {value:?} is not supported")
            }
            ErrorType::DuplicatedParameter => write!(f, "Duplicated argument"),
            ErrorType::UndeclaredCapability(value) => {
                write!(f, "Undeclared capability '{value}'")
            }
            ErrorType::MissingTag(value) => write!(f, "Missing tag {value:?}"),
        }?;

        write!(
            f,
            " at line {}, column {}.",
            self.line_num(),
            self.line_pos()
        )
    }
}

impl Display for RuntimeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RuntimeError::TooManyIncludes => write!(f, ""),
            RuntimeError::InvalidInstruction(value) => write!(
                f,
                "Script executed invalid instruction {:?} at line {}, column {}.",
                value.name(),
                value.line_pos(),
                value.line_num()
            ),
            RuntimeError::ScriptErrorMessage(value) => {
                write!(f, "Script reported error {value:?}.")
            }
            RuntimeError::CapabilityNotAllowed(value) => {
                write!(f, "Capability '{value}' has been disabled.")
            }
            RuntimeError::CapabilityNotSupported(value) => {
                write!(f, "Capability '{value}' not supported.")
            }
            RuntimeError::CPULimitReached => write!(
                f,
                "Script exceeded the maximum number of instructions allowed to execute."
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use crate::Compiler;

    #[test]
    fn parse_rfc() {
        let mut test_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        test_dir.push("tests");
        test_dir.push("rfcs");
        let mut tests_run = 0;

        let mut compiler = Compiler::new().with_max_nested_foreverypart(10);

        compiler
            .register_plugin("plugin1")
            .with_tag("tag1")
            .with_tag("tag2")
            .with_tagged_string_argument("string_arg")
            .with_number_argument()
            .with_string_array_argument();

        compiler
            .register_plugin("plugin2")
            .with_tagged_number_array_argument("array_arg")
            .with_regex_array_argument();

        for file_name in fs::read_dir(&test_dir).unwrap() {
            let mut file_name = file_name.unwrap().path();
            if file_name.extension().map_or(false, |e| e == "sieve") {
                println!("Parsing {}", file_name.display());

                /*if !file_name
                    .file_name()
                    .unwrap()
                    .to_str()
                    .unwrap()
                    .contains("plugins")
                {
                    let test = "true";
                    continue;
                }*/

                let script = fs::read(&file_name).unwrap();
                file_name.set_extension("json");
                let expected_result = fs::read(&file_name).unwrap();

                tests_run += 1;

                let sieve = compiler.compile(&script).unwrap();
                let json_sieve = serde_json::to_string_pretty(
                    &sieve
                        .instructions
                        .into_iter()
                        .enumerate()
                        .collect::<Vec<_>>(),
                )
                .unwrap();

                if json_sieve.as_bytes() != expected_result {
                    file_name.set_extension("failed");
                    fs::write(&file_name, json_sieve.as_bytes()).unwrap();
                    panic!("Test failed, parsed sieve saved to {}", file_name.display());
                }
            }
        }

        assert!(
            tests_run > 0,
            "Did not find any tests to run in folder {}.",
            test_dir.display()
        );
    }
}
