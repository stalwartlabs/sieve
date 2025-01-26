/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use serde::{Deserialize, Serialize};

use crate::compiler::grammar::instruction::CompilerState;
use crate::compiler::CompileError;
use crate::compiler::Value;

use crate::compiler::grammar::test::Test;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestValidExtList {
    pub list_names: Vec<Value>,
    pub is_not: bool,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_valid_ext_list(&mut self) -> Result<Test, CompileError> {
        Ok(Test::ValidExtList(TestValidExtList {
            list_names: self.parse_strings(false)?,
            is_not: false,
        }))
    }
}
