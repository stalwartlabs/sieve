/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    CompileError, ErrorType, Value,
    grammar::instruction::{CompilerState, MapLocalVars},
    lexer::{Token, word::Word},
};

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestDuplicate {
    pub handle: Option<Value>,
    pub dup_match: DupMatch,
    pub seconds: Option<u64>,
    pub last: bool,
    pub is_not: bool,
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
#[repr(u8)]
pub(crate) enum DupMatch {
    Header(Value) = 0,
    UniqueId(Value) = 1,
    Default = 2,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_duplicate(&mut self) -> Result<Test, CompileError> {
        let mut handle = None;
        let mut dup_match = DupMatch::Default;
        let mut seconds = None;
        let mut last = false;

        while let Some(token_info) = self.tokens.peek() {
            let token_info = token_info?;
            let line_num = token_info.line_num;
            let line_pos = token_info.line_pos;

            match token_info.token {
                Token::Tag(Word::Handle) => {
                    self.validate_argument(1, None, line_num, line_pos)?;
                    self.tokens.next();
                    handle = self.parse_string()?.into();
                }
                Token::Tag(Word::Header) => {
                    self.validate_argument(2, None, line_num, line_pos)?;
                    self.tokens.next();
                    let header = self.parse_raw_string()?;
                    dup_match = match self.resolve_header_name(header) {
                        Some(header) => DupMatch::Header(header),
                        None => {
                            return Err(self
                                .tokens
                                .unwrap_next()?
                                .custom(ErrorType::InvalidHeaderName));
                        }
                    };
                }
                Token::Tag(Word::UniqueId) => {
                    self.validate_argument(2, None, line_num, line_pos)?;
                    self.tokens.next();
                    dup_match = DupMatch::UniqueId(self.parse_string()?);
                }
                Token::Tag(Word::Seconds) => {
                    self.validate_argument(3, None, line_num, line_pos)?;
                    self.tokens.next();
                    seconds = (self.tokens.expect_number(u64::MAX as usize)? as u64).into();
                }
                Token::Tag(Word::Last) => {
                    self.validate_argument(4, None, line_num, line_pos)?;
                    self.tokens.next();
                    last = true;
                }
                _ => break,
            }
        }

        Ok(Test::Duplicate(Box::new(TestDuplicate {
            handle,
            dup_match,
            seconds,
            last,
            is_not: false,
        })))
    }
}

impl MapLocalVars for DupMatch {
    fn map_local_vars(&mut self, last_id: u16) {
        match self {
            DupMatch::Header(header) => header.map_local_vars(last_id),
            DupMatch::UniqueId(unique_id) => unique_id.map_local_vars(last_id),
            DupMatch::Default => {}
        }
    }
}
