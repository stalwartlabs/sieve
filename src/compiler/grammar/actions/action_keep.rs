/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
        Capability,
    },
    lexer::{word::Word, Token},
    CompileError, Value,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Keep {
    pub flags: Vec<Value>,
}

impl CompilerState<'_> {
    pub(crate) fn parse_keep(&mut self) -> Result<(), CompileError> {
        let cmd = Instruction::Keep(Keep {
            flags: match self.tokens.peek().map(|r| r.map(|t| &t.token)) {
                Some(Ok(Token::Tag(Word::Flags))) => {
                    let token_info = self.tokens.next().unwrap().unwrap();
                    self.validate_argument(
                        0,
                        Capability::Imap4Flags.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    self.parse_strings(false)?
                }
                _ => Vec::new(),
            },
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
