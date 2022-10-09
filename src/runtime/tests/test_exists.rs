use mail_parser::Message;

use crate::{compiler::grammar::tests::test_exists::TestExists, Context};

use super::mime::SubpartIterator;

impl TestExists {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) -> bool {
        let header_names = ctx.parse_header_names(&self.header_names);
        let parts = [ctx.part];
        let mut part_iter = SubpartIterator::new(message, &parts, self.mime_anychild);
        let mut result = false;

        'outer: while let Some(message_part) = part_iter.next() {
            for header_name in &header_names {
                if !message_part.headers.iter().any(|h| &h.name == header_name) {
                    continue 'outer;
                }
            }
            result = true;
            break;
        }

        result ^ self.is_not
    }
}
