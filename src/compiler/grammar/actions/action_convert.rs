/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::{
    grammar::{
        instruction::{CompilerState, Instruction},
        test::Test,
    },
    CompileError, Value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct Convert {
    pub from_media_type: Value,
    pub to_media_type: Value,
    pub transcoding_params: Vec<Value>,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_convert(&mut self) -> Result<Test, CompileError> {
        Ok(Test::Convert(Convert {
            from_media_type: self.parse_string()?,
            to_media_type: self.parse_string()?,
            transcoding_params: self.parse_strings(false)?,
            is_not: false,
        }))
    }

    pub(crate) fn parse_convert(&mut self) -> Result<(), CompileError> {
        let cmd = Instruction::Convert(Convert {
            from_media_type: self.parse_string()?,
            to_media_type: self.parse_string()?,
            transcoding_params: self.parse_strings(false)?,
            is_not: false,
        });
        self.instructions.push(cmd);
        Ok(())
    }
}
