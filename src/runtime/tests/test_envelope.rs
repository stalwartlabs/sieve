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

pub(crate) fn parse_envelope_address(addr: &str) -> Option<&str> {
    let addr = addr.as_bytes();
    let mut addr_start_pos = 0;
    let mut addr_end_pos = addr.len();
    let mut last_ch = 0;
    let mut at_pos = 0;
    let mut has_bracket = false;
    let mut in_path = false;

    if addr.is_empty() {
        return "".into();
    }

    for (pos, &ch) in addr.iter().enumerate() {
        match ch {
            b'<' => {
                if pos == 0 {
                    addr_start_pos = pos + 1;
                    has_bracket = true;
                } else {
                    return None;
                }
            }
            b'>' => {
                if has_bracket && pos == addr.len() - 1 {
                    if addr.len() > 2 {
                        has_bracket = false;
                        addr_end_pos = pos;
                    } else {
                        // <>
                        return "".into();
                    }
                } else {
                    return None;
                }
            }
            b':' => {
                if at_pos != 0 {
                    at_pos = 0;
                    addr_start_pos = pos + 1;
                    in_path = false;
                } else {
                    return None;
                }
            }
            b',' => {
                if at_pos != 0 {
                    at_pos = 0;
                    in_path = true;
                } else {
                    return None;
                }
            }
            b'@' => {
                if at_pos == 0 && pos != addr.len() - 1 {
                    at_pos = pos;
                } else {
                    return None;
                }
            }
            b'.' => {
                if (at_pos != 0 && last_ch == b'.') || last_ch == b'@' {
                    return None;
                }
            }
            _ => {
                if ch.is_ascii_whitespace() || !ch.is_ascii() {
                    return None;
                }
            }
        }

        last_ch = ch;
    }

    if !has_bracket && !in_path && at_pos > addr_start_pos && addr_end_pos - 1 > at_pos {
        std::str::from_utf8(&addr[addr_start_pos..addr_end_pos])
            .unwrap()
            .into()
    } else {
        match addr.get(addr_start_pos..addr_end_pos) {
            Some(addr) if at_pos == 0 && addr.eq_ignore_ascii_case(b"mailer-daemon") => {
                std::str::from_utf8(addr).unwrap().into()
            }
            _ => None,
        }
    }
}
