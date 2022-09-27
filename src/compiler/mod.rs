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
    InvalidUnicodeSequence(u32),
    InvalidUtf8String,
    UnterminatedString,
    UnterminatedComment,
    UnterminatedMultiline,
    UnterminatedBlock,
    ScriptTooLong,
    StringTooLong,
    VariableTooLong,
    UnexpectedToken(String),
    MissingParameters(String),
    UnexpectedEOF,
    TooManyNestedBlocks,
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

impl From<TokenInfo> for CompileError {
    fn from(token: TokenInfo) -> Self {
        CompileError {
            line_num: token.line_num,
            line_pos: token.line_pos,
            error_type: ErrorType::UnexpectedToken(token.token.to_string()),
        }
    }
}
