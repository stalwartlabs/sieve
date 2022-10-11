use mail_parser::Message;

use crate::{compiler::grammar::tests::test_exists::TestExists, Context};

use super::mime::SubpartIterator;

impl TestExists {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) -> bool {
        let header_names = ctx.parse_header_names(&self.header_names);
        let mut header_exists = vec![false; header_names.len()];
        let parts = [ctx.part];
        let mut part_iter = SubpartIterator::new(ctx, message, &parts, self.mime_anychild);
        let mut result = false;

        while let Some((part_id, message_part)) = part_iter.next() {
            for (pos, header_name) in header_names.iter().enumerate() {
                if !header_exists[pos] {
                    if message_part
                        .headers
                        .iter()
                        .any(|h| !ctx.is_header_deleted(h.offset_field) && &h.name == header_name)
                    {
                        header_exists[pos] = true;
                    } else {
                        for header in ctx.get_inserted_headers(part_id) {
                            if &header.name == header_name {
                                header_exists[pos] = true;
                                break;
                            }
                        }
                    }
                }
            }

            if header_exists.iter().all(|v| *v) {
                result = true;
                break;
            }
        }

        result ^ self.is_not
    }
}
