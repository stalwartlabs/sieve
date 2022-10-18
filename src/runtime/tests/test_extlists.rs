use crate::{compiler::grammar::tests::test_extlists::TestValidExtList, Context};

use super::TestResult;

impl TestValidExtList {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut num_valid = 0;

        for list in &self.list_names {
            if ctx.runtime.valid_ext_lists.contains(&ctx.eval_string(list)) {
                num_valid += 1;
            }
        }

        TestResult::Bool((num_valid == self.list_names.len()) ^ self.is_not)
    }
}
