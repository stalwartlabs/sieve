use crate::{compiler::grammar::tests::test_size::TestSize, Context};

impl TestSize {
    pub(crate) fn exec(&self, ctx: &Context) -> bool {
        (if self.over {
            ctx.message_size > self.limit
        } else {
            ctx.message_size < self.limit
        }) ^ self.is_not
    }
}
