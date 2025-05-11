/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */



use crate::compiler::grammar::instruction::{CompilerState, Instruction};
use crate::compiler::grammar::Capability;
use crate::compiler::CompileError;
use crate::compiler::Value;

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
pub(crate) struct TestIhave {
    pub capabilities: Vec<Capability>,
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
pub(crate) struct Error {
    pub message: Value,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_ihave(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Ihave(TestIhave {
            capabilities: self
                .parse_static_strings()?
                .into_iter()
                .map(|n| n.into())
                .collect(),
            is_not: false,
        }))
    }

    pub(crate) fn parse_error(&mut self) -> Result<(), CompileError> {
        let cmd = Instruction::Error(Error {
            message: self.parse_string()?,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
