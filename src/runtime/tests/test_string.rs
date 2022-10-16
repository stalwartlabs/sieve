use crate::{
    compiler::grammar::{tests::test_string::TestString, MatchType},
    Context,
};

impl TestString {
    pub(crate) fn exec(&self, ctx: &mut Context, empty_is_null: bool) -> bool {
        let mut result = false;
        if let MatchType::Count(match_type) = &self.match_type {
            let num_items = self
                .source
                .iter()
                .filter(|x| !ctx.eval_string(x).is_empty())
                .count() as f64;
            if !empty_is_null || num_items > 0.0 {
                for key in &self.key_list {
                    if match_type.cmp_num(num_items, ctx.eval_string(key).as_ref()) {
                        result = true;
                        break;
                    }
                }
            }
        } else {
            let mut captured_values = Vec::new();
            let sources = ctx.eval_strings(&self.source);

            for key in &self.key_list {
                let key = ctx.eval_string(key);
                for source in &sources {
                    if !empty_is_null || !source.is_empty() {
                        result = match &self.match_type {
                            MatchType::Is => self.comparator.is(source.as_ref(), key.as_ref()),
                            MatchType::Contains => {
                                self.comparator.contains(source.as_ref(), key.as_ref())
                            }
                            MatchType::Value(relation) => {
                                self.comparator
                                    .relational(relation, source.as_ref(), key.as_ref())
                            }
                            MatchType::Matches(capture_positions) => self.comparator.matches(
                                source.as_ref(),
                                key.as_ref(),
                                *capture_positions,
                                &mut captured_values,
                            ),
                            MatchType::Regex(capture_positions) => self.comparator.regex(
                                source.as_ref(),
                                key.as_ref(),
                                *capture_positions,
                                &mut captured_values,
                            ),
                            _ => false,
                        };

                        if result {
                            break;
                        }
                    }
                }
            }

            if !captured_values.is_empty() {
                ctx.set_match_variables(captured_values);
            }
        }

        result ^ self.is_not
    }
}
