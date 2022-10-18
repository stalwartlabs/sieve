use crate::{
    compiler::grammar::{
        tests::test_notify::{TestNotifyMethodCapability, TestValidNotifyMethod},
        MatchType,
    },
    Context, URI,
};

use super::TestResult;

impl TestValidNotifyMethod {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut num_valid = 0;

        for uri in &self.notification_uris {
            if let Some(uri) = URI::parse(ctx.eval_string(uri).as_ref()) {
                if ctx.runtime.valid_notification_uris.contains(&uri.scheme()) {
                    num_valid += 1;
                }
            }
        }

        TestResult::Bool((num_valid == self.notification_uris.len()) ^ self.is_not)
    }
}

impl TestNotifyMethodCapability {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        if !ctx
            .eval_string(&self.notification_capability)
            .eq_ignore_ascii_case("online")
            || !URI::parse(ctx.eval_string(&self.notification_uri).as_ref()).map_or(false, |uri| {
                ctx.runtime.valid_notification_uris.contains(&uri.scheme())
            })
        {
            return TestResult::Bool(false ^ self.is_not);
        }

        if let MatchType::Count(rel_match) = &self.match_type {
            for key in &self.key_list {
                if rel_match.cmp_num(1.0, ctx.eval_string(key).as_ref()) {
                    return TestResult::Bool(true ^ self.is_not);
                }
            }
        } else {
            for key in &self.key_list {
                let key = ctx.eval_string(key);
                if match &self.match_type {
                    MatchType::Is => self.comparator.is("maybe", key.as_ref()),
                    MatchType::Contains => self.comparator.contains("maybe", key.as_ref()),
                    MatchType::Value(relation) => {
                        self.comparator.relational(relation, "maybe", key.as_ref())
                    }
                    MatchType::Matches(_) => {
                        self.comparator
                            .matches("maybe", key.as_ref(), 0, &mut Vec::new())
                    }
                    MatchType::Regex(_) => {
                        self.comparator
                            .regex("maybe", key.as_ref(), 0, &mut Vec::new())
                    }
                    _ => false,
                } {
                    return TestResult::Bool(true ^ self.is_not);
                }
            }
        }

        TestResult::Bool(false ^ self.is_not)
    }
}
