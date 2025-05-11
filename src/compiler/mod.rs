/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use self::{
    grammar::{AddressPart, Capability},
    lexer::tokenizer::TokenInfo,
};
use crate::{runtime::RuntimeError, Compiler, Envelope, FunctionMap};
use ahash::AHashMap;
use arc_swap::ArcSwap;
use mail_parser::HeaderName;
use std::{borrow::Cow, fmt::Display, sync::Arc};

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
    ContinueOutsideLoop,
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[cfg_attr(
    feature = "rkyv",
    rkyv(serialize_bounds(
        __S: rkyv::ser::Writer + rkyv::ser::Allocator,
        __S::Error: rkyv::rancor::Source,
    ))
)]
#[cfg_attr(
    feature = "rkyv",
    rkyv(deserialize_bounds(__D::Error: rkyv::rancor::Source))
)]
#[cfg_attr(
    feature = "rkyv",
    rkyv(bytecheck(
        bounds(
            __C: rkyv::validation::ArchiveContext,
        )
    ))
)]
pub(crate) enum Value {
    Text(Arc<String>),
    Number(Number),
    Variable(VariableType),
    Regex(Regex),
    List(#[cfg_attr(feature = "rkyv", rkyv(omit_bounds))] Vec<Value>),
}

#[derive(Debug, Clone)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub struct Regex {
    #[cfg_attr(feature = "rkyv", rkyv(with = rkyv::with::Skip))]
    #[cfg_attr(any(test, feature = "serde"), serde(skip, default))]
    pub regex: LazyRegex,
    pub expr: String,
}

#[derive(Debug, Clone)]
pub struct LazyRegex(pub Arc<ArcSwap<Option<fancy_regex::Regex>>>);

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum VariableType {
    Local(usize),
    Match(usize),
    Global(String),
    Environment(String),
    Envelope(Envelope),
    Header(HeaderVariable),
    Part(MessagePart),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub struct Transform {
    pub variable: Box<VariableType>,
    pub functions: Vec<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub struct HeaderVariable {
    pub name: Vec<HeaderName<'static>>,
    pub part: HeaderPart,
    pub index_hdr: i32,
    pub index_part: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum MessagePart {
    TextBody(bool),
    HtmlBody(bool),
    Contents,
    Raw,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum HeaderPart {
    Text,
    Date,
    Id,
    Address(AddressPart),
    ContentType(ContentTypePart),
    Received(ReceivedPart),
    Raw,
    RawName,
    Exists,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum ContentTypePart {
    Type,
    Subtype,
    Attribute(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
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

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub enum ReceivedHostname {
    Name,
    Ip,
    Any,
}

#[derive(Debug, Clone, Copy)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
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

impl Default for LazyRegex {
    fn default() -> Self {
        Self(Arc::new(ArcSwap::new(Arc::new(None))))
    }
}

impl Regex {
    pub fn new(expr: String, regex: fancy_regex::Regex) -> Self {
        Self {
            expr,
            regex: LazyRegex(Arc::new(ArcSwap::new(Arc::new(Some(regex))))),
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
            ErrorType::ContinueOutsideLoop => write!(f, "Continue used outside of while loop"),
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

        let compiler = Compiler::new().with_max_nested_foreverypart(10);

        for file_name in fs::read_dir(&test_dir).unwrap() {
            let mut file_name = file_name.unwrap().path();
            if file_name.extension().is_some_and(|e| e == "sieve") {
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
