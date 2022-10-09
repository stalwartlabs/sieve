use mail_parser::Message;

use crate::compiler::grammar::tests::test_size::TestSize;

impl TestSize {
    pub(crate) fn exec(&self, message: &Message) -> bool {
        (if self.over {
            message.raw_message.len() > self.limit
        } else {
            message.raw_message.len() < self.limit
        }) ^ self.is_not
    }
}
