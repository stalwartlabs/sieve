/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::instruction::{CompilerState, Instruction},
    CompileError, Value,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct Reject {
    pub ereject: bool,
    pub reason: Value,
}

impl CompilerState<'_> {
    pub(crate) fn parse_reject(&mut self, ereject: bool) -> Result<(), CompileError> {
        let cmd = Instruction::Reject(Reject {
            ereject,
            reason: self.parse_string()?,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
