use std::borrow::Cow;

use crate::Compiler;

use self::lexer::tokenizer::TokenInfo;

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
    UnsupportedComparator(String),
    InvalidGrammar(Cow<'static, str>),
}

impl Default for Compiler {
    fn default() -> Self {
        Self::new()
    }
}

impl Compiler {
    pub fn new() -> Self {
        Compiler {
            max_script_len: 1024 * 1024,
            max_string_len: 1024 * 1024,
            max_variable_len: 32,
            max_nested_blocks: 15,
            max_nested_tests: 15,
        }
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
                    &sieve.commands.into_iter().enumerate().collect::<Vec<_>>(),
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
