use crate::{compiler::grammar::tests::test_size::TestSize, Context};

use super::TestResult;

impl TestSize {
    pub(crate) fn exec(&self, ctx: &Context) -> TestResult {
        TestResult::Bool(
            (if self.over {
                ctx.message_size > self.limit
            } else {
                ctx.message_size < self.limit
            }) ^ self.is_not,
        )
    }
}
