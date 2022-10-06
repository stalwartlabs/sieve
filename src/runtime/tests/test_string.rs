use crate::{
    compiler::grammar::{tests::test_string::TestString, MatchType},
    Context,
};

impl TestString {
    pub(crate) fn exec(&self, ctx: &mut Context, is_not: bool) -> bool {
        let mut result = false;
        if let MatchType::Count(match_type) = &self.match_type {
            let num_items = self
                .source
                .iter()
                .filter(|x| !ctx.eval_string(x).is_empty())
                .count() as f64;
            for key in &self.key_list {
                if match_type.cmp_num(num_items, ctx.eval_string(key).as_ref()) ^ is_not {
                    result = true;
                    break;
                }
            }
        } else {
            let mut matched_values = Vec::new();
            let sources = ctx.eval_strings(&self.source);

            for key in &self.key_list {
                let key = ctx.eval_string(key);
                for source in &sources {
                    if self.match_type.match_value(
                        source.as_ref(),
                        key.as_ref(),
                        &self.comparator,
                        &mut matched_values,
                    ) ^ is_not
                    {
                        result = true;
                        break;
                    }
                }
            }

            if !matched_values.is_empty() {
                ctx.set_match_variables(matched_values);
            }
        }

        result
    }
}

/*

use crate::{compiler::grammar::tests::test_string::TestString, Context};

impl TestString {
    pub(crate) fn exec(&self, ctx: &mut Context) -> bool {
        false
    }
}

*/
