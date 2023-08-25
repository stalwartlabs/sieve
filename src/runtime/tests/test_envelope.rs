/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
*/

use mail_parser::DateTime;

use crate::{
    compiler::{
        grammar::{tests::test_envelope::TestEnvelope, MatchType},
        Number,
    },
    runtime::Variable,
    Context, Envelope, Event,
};

use super::TestResult;

impl TestEnvelope {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let key_list = ctx.eval_values(&self.key_list);

        let result = match &self.match_type {
            MatchType::Is | MatchType::Contains => {
                let is_is = matches!(&self.match_type, MatchType::Is);

                ctx.find_envelopes(self, |value| {
                    for key in &key_list {
                        if is_is {
                            if self.comparator.is(&Variable::from(value), key) {
                                return true;
                            }
                        } else if self.comparator.contains(value, key.to_cow().as_ref()) {
                            return true;
                        }
                    }

                    false
                })
            }
            MatchType::Value(rel_match) => ctx.find_envelopes(self, |value| {
                for key in &key_list {
                    if self
                        .comparator
                        .relational(rel_match, &Variable::from(value), key)
                    {
                        return true;
                    }
                }

                false
            }),
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_positions = Vec::new();
                let is_matches = matches!(&self.match_type, MatchType::Matches(_));

                let result = ctx.find_envelopes(self, |value| {
                    for (pattern_expr, pattern) in key_list.iter().zip(self.key_list.iter()) {
                        if is_matches {
                            if self.comparator.matches(
                                value,
                                pattern_expr.to_cow().as_ref(),
                                *capture_positions,
                                &mut captured_positions,
                            ) {
                                return true;
                            }
                        } else if self.comparator.regex(
                            pattern,
                            pattern_expr,
                            value,
                            *capture_positions,
                            &mut captured_positions,
                        ) {
                            return true;
                        }
                    }

                    false
                });

                if !captured_positions.is_empty() {
                    ctx.set_match_variables(captured_positions);
                }

                result
            }

            MatchType::Count(rel_match) => {
                let mut count = 0;

                ctx.find_envelopes(self, |value| {
                    if !value.is_empty() {
                        count += 1;
                    }

                    false
                });

                let mut result = false;
                for key in &key_list {
                    if rel_match.cmp(&Number::from(count), &key.to_number()) {
                        result = true;
                        break;
                    }
                }
                result
            }
            MatchType::List => {
                let mut values: Vec<String> = Vec::new();

                ctx.find_envelopes(self, |value| {
                    if !value.is_empty() && !values.iter().any(|v| v.eq(value)) {
                        values.push(value.to_string());
                    }

                    false
                });

                if !values.is_empty() {
                    return TestResult::Event {
                        event: Event::ListContains {
                            lists: ctx.eval_values_owned(&self.key_list),
                            values,
                            match_as: self.comparator.as_match(),
                        },
                        is_not: self.is_not,
                    };
                }

                false
            }
        };
        TestResult::Bool(result ^ self.is_not)
    }
}

impl<'x> Context<'x> {
    fn find_envelopes(
        &self,
        test_envelope: &TestEnvelope,
        mut cb: impl FnMut(&str) -> bool,
    ) -> bool {
        for (name, value) in &self.envelope {
            if test_envelope.envelope_list.contains(name)
                && match name {
                    Envelope::From | Envelope::To | Envelope::Orcpt => {
                        if let Some(value) = test_envelope
                            .address_part
                            .eval_string(value.to_cow().as_ref())
                        {
                            cb(value)
                        } else {
                            false
                        }
                    }
                    Envelope::ByTimeAbsolute if test_envelope.zone.is_some() => {
                        if let Some(dt) = DateTime::parse_rfc3339(value.to_cow().as_ref()) {
                            cb(&dt.to_timezone(test_envelope.zone.unwrap()).to_rfc3339())
                        } else {
                            cb("")
                        }
                    }
                    _ => cb(value.to_cow().as_ref()),
                }
            {
                return true;
            }
        }
        false
    }
}

pub fn parse_envelope_address(addr: &str) -> Option<&str> {
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
