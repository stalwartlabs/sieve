use crate::{
    compiler::grammar::{
        actions::action_set::Variable, tests::test_hasflag::TestHasFlag, MatchType,
    },
    Context,
};

impl TestHasFlag {
    pub(crate) fn exec(&self, ctx: &mut Context) -> bool {
        let mut variable_list_ = None;
        let variable_list = if !self.variable_list.is_empty() {
            &self.variable_list
        } else {
            variable_list_.get_or_insert_with(|| vec![Variable::Global("__flags".to_string())])
        };

        let result = if let MatchType::Count(rel_match) = &self.match_type {
            let mut flag_count = 0;
            for variable in variable_list {
                match ctx.get_variable(variable) {
                    Some(flags) if !flags.is_empty() => {
                        flag_count += flags.split(' ').count();
                    }
                    _ => (),
                }
            }

            let mut result = false;
            for key in &self.flags {
                if rel_match.cmp_num(flag_count as f64, ctx.eval_string(key).as_ref()) {
                    result = true;
                    break;
                }
            }
            result
        } else {
            let mut captured_values = Vec::new();
            let result = ctx.tokenize_flags(&self.flags, |check_flag| {
                for variable in variable_list {
                    match ctx.get_variable(variable) {
                        Some(flags) if !flags.is_empty() => {
                            for flag in flags.split(' ') {
                                if match &self.match_type {
                                    MatchType::Is => self.comparator.is(flag, check_flag),
                                    MatchType::Contains => {
                                        self.comparator.contains(flag, check_flag)
                                    }
                                    MatchType::Value(rel_match) => {
                                        self.comparator.relational(rel_match, flag, check_flag)
                                    }
                                    MatchType::Matches(capture_positions) => {
                                        self.comparator.matches(
                                            flag,
                                            check_flag,
                                            *capture_positions,
                                            &mut captured_values,
                                        )
                                    }
                                    MatchType::Regex(capture_positions) => self.comparator.matches(
                                        flag,
                                        check_flag,
                                        *capture_positions,
                                        &mut captured_values,
                                    ),
                                    MatchType::Count(_) | MatchType::List => false,
                                } {
                                    return true;
                                }
                            }
                        }
                        _ => (),
                    }
                }
                false
            });
            if !captured_values.is_empty() {
                ctx.set_match_variables(captured_values);
            }
            result
        };

        result ^ self.is_not
    }
}
