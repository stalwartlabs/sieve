use crate::{
    compiler::{
        grammar::{tests::test_envelope::TestEnvelope, MatchType},
        lexer::string::StringItem,
    },
    runtime::ENVELOPE,
    Context, Envelope,
};

impl TestEnvelope {
    pub(crate) fn exec(&self, ctx: &mut Context) -> bool {
        let key_list = ctx.eval_strings(&self.key_list);
        let envelope_list = ctx.parse_envelope_names(&self.envelope_list);

        //TODO implement zone
        (match &self.match_type {
            MatchType::Is | MatchType::Contains => {
                let is_is = matches!(&self.match_type, MatchType::Is);
                let mut result = false;

                'outer: for (name, value) in &ctx.envelope {
                    if envelope_list.contains(name) {
                        if let Some(value) = self.address_part.eval(value.as_ref()) {
                            for key in &key_list {
                                if is_is {
                                    if self.comparator.is(value, key.as_ref()) {
                                        result = true;
                                        break 'outer;
                                    }
                                } else if self.comparator.contains(value, key.as_ref()) {
                                    result = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                result
            }
            MatchType::Value(rel_match) => {
                let mut result = false;

                'outer: for (name, value) in &ctx.envelope {
                    if envelope_list.contains(name) {
                        if let Some(value) = self.address_part.eval(value.as_ref()) {
                            for key in &key_list {
                                if self.comparator.relational(rel_match, value, key.as_ref()) {
                                    result = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }
                result
            }
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_positions = Vec::new();
                let is_matches = matches!(&self.match_type, MatchType::Matches(_));
                let mut result = false;

                'outer: for (name, value) in &ctx.envelope {
                    if envelope_list.contains(name) {
                        if let Some(value) = self.address_part.eval(value.as_ref()) {
                            for key in &key_list {
                                if is_matches {
                                    if self.comparator.matches(
                                        value,
                                        key.as_ref(),
                                        *capture_positions,
                                        &mut captured_positions,
                                    ) {
                                        result = true;
                                        break 'outer;
                                    }
                                } else if self.comparator.regex(
                                    value,
                                    key.as_ref(),
                                    *capture_positions,
                                    &mut captured_positions,
                                ) {
                                    result = true;
                                    break 'outer;
                                }
                            }
                        }
                    }
                }

                if !captured_positions.is_empty() {
                    ctx.set_match_variables(captured_positions);
                }

                result
            }

            MatchType::Count(rel_match) => {
                let mut count = 0;

                for (name, value) in &ctx.envelope {
                    if envelope_list.contains(name) {
                        if let Some(value) = self.address_part.eval(value.as_ref()) {
                            if !value.is_empty() {
                                count += 1;
                            }
                        }
                    }
                }

                let mut result = false;
                for key in &key_list {
                    if rel_match.cmp_num(count as f64, key.as_ref()) {
                        result = true;
                        break;
                    }
                }
                result
            }
            MatchType::List => false, //TODO: Implement
        }) ^ self.is_not
    }
}

impl<'x> Context<'x> {
    pub(crate) fn parse_envelope_names<'z: 'y, 'y>(
        &'z self,
        envelope_names: &'y [StringItem],
    ) -> Vec<Envelope<'y>> {
        let mut result = Vec::with_capacity(envelope_names.len());
        for envelope_name in envelope_names {
            let envelope_name = self.eval_string(envelope_name);
            result.push(if let Some(envelope) = ENVELOPE.get(&envelope_name) {
                envelope.clone()
            } else {
                Envelope::Other(envelope_name)
            });
        }
        result
    }
}
