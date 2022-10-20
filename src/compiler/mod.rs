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

use std::borrow::Cow;

use crate::Compiler;

use self::{grammar::Capability, lexer::tokenizer::TokenInfo};

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
    InvalidUtf8String,
    UnterminatedString,
    UnterminatedComment,
    UnterminatedMultiline,
    UnterminatedBlock,
    ScriptTooLong,
    StringTooLong,
    VariableTooLong,
    UnexpectedToken {
        expected: Cow<'static, str>,
        found: String,
    },
    UnexpectedEOF,
    TooManyNestedBlocks,
    TooManyNestedTests,
    TooManyIncludes,
    UnsupportedComparator(String),
    InvalidGrammar(Cow<'static, str>),
    DuplicatedParameter,
    UndeclaredCapability(Capability),
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            max_script_size: 1024 * 1024,
            max_string_size: 1024 * 1024,
            max_variable_size: 32,
            max_nested_blocks: 15,
            max_nested_tests: 15,
            max_nested_foreverypart: 3,
            max_match_variables: 30,
            max_local_variables: 128,
            max_header_size: 1024,
            max_includes: 6,
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

    pub fn set_max_variable_size(&mut self, size: usize) {
        self.max_variable_size = size;
    }

    pub fn with_max_variable_size(mut self, size: usize) -> Self {
        self.max_variable_size = size;
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

    pub fn invalid(self, reason: impl Into<Cow<'static, str>>) -> CompileError {
        CompileError {
            line_num: self.line_num,
            line_pos: self.line_pos,
            error_type: ErrorType::InvalidGrammar(reason.into()),
        }
    }

    pub fn invalid_utf8(self) -> CompileError {
        CompileError {
            line_num: self.line_num,
            line_pos: self.line_pos,
            error_type: ErrorType::InvalidUtf8String,
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

        for file_name in fs::read_dir(&test_dir).unwrap() {
            let mut file_name = file_name.unwrap().path();
            if file_name.extension().map_or(false, |e| e == "sieve") {
                println!("Parsing {}...", file_name.display());
                let script = fs::read(&file_name).unwrap();
                file_name.set_extension("json");
                //let expected_result = fs::read(&file_name).unwrap();

                tests_run += 1;

                let sieve = Compiler::new().compile(&script).unwrap();
                let json_sieve = serde_json::to_string_pretty(
                    &sieve
                        .instructions
                        .into_iter()
                        .enumerate()
                        .collect::<Vec<_>>(),
                )
                .unwrap();

                fs::write(&file_name, json_sieve.as_bytes()).unwrap();

                /*if json_sieve.as_bytes() != expected_result {
                    file_name.set_extension("failed");
                    fs::write(&file_name, json_sieve.as_bytes()).unwrap();
                    panic!("Test failed, parsed sieve saved to {}", file_name.display());
                }*/
            }
        }

        assert!(
            tests_run > 0,
            "Did not find any tests to run in folder {}.",
            test_dir.display()
        );
    }
}
