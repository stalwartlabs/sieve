/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{
    Context, Envelope, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{AddressPart, Comparator, MatchType},
    },
    runtime::{RuntimeError, handler::Handler},
};
use mail_parser::DateTime;
use smallvec::SmallVec;

type EnvelopeList = SmallVec<[Envelope; 4]>;

impl<'x> Context<'x> {
    pub(crate) fn test_envelope<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestEnvelope,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let key_list = self.eval_keys(script, test.key_list)?;
        let envelope_list: EnvelopeList = script
            .recs(test.envelope_list)?
            .map(|rec| Envelope::from_code(rec.b))
            .collect();
        let address_part = AddressPart::from_code(test.address_part);
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();

        let result = match &match_type {
            MatchType::Is | MatchType::Contains | MatchType::Value(_) => {
                self.find_envelopes(&envelope_list, address_part, test.zone, |value| {
                    key_list.iter().any(|key| match &match_type {
                        MatchType::Is => comparator.is(&value, &key.value),
                        MatchType::Contains => {
                            comparator.contains(value, key.value.to_string().as_ref())
                        }
                        MatchType::Value(rel_match) => {
                            comparator.relational(rel_match, &value, &key.value)
                        }
                        _ => false,
                    })
                })
            }
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_values = Vec::new();
                let is_matches = matches!(&match_type, MatchType::Matches(_));
                let to_lower = comparator.is_casemap();
                let mut error = None;
                let result =
                    self.find_envelopes(&envelope_list, address_part, test.zone, |value| {
                        for key in &key_list {
                            let matched = if is_matches {
                                self.glob_matches(
                                    script,
                                    to_lower,
                                    key,
                                    value,
                                    *capture_positions,
                                    &mut captured_values,
                                )
                            } else {
                                self.regex_matches(
                                    script,
                                    key,
                                    value,
                                    *capture_positions,
                                    &mut captured_values,
                                )
                            };
                            match matched {
                                Ok(true) => return true,
                                Ok(false) => (),
                                Err(err) => {
                                    error = Some(err);
                                    return true;
                                }
                            }
                        }
                        false
                    });
                if let Some(err) = error {
                    return Err(err);
                }
                if !captured_values.is_empty() {
                    self.set_match_variables(captured_values);
                }
                result
            }
            MatchType::Count(rel_match) => {
                let mut count: i64 = 0;
                self.find_envelopes(&envelope_list, address_part, test.zone, |value| {
                    if !value.is_empty() {
                        count += 1;
                    }
                    false
                });

                key_list
                    .iter()
                    .any(|key| rel_match.cmp(&Number::from(count), &key.value.to_number()))
            }
            MatchType::List => {
                let mut values: Vec<&str> = Vec::new();
                self.find_envelopes(&envelope_list, address_part, test.zone, |value| {
                    if !value.is_empty() && !values.contains(&value) {
                        values.push(self.alloc_str(value));
                    }
                    false
                });

                if !values.is_empty() {
                    let lists: SmallVec<[&str; 4]> = key_list
                        .iter()
                        .map(|key| self.intern_cow(key.value.clone().into_string()))
                        .collect();
                    return TestResult::from_reply(
                        handler.list_contains(self, &lists, &values, comparator.as_match()),
                        test.is_not,
                    );
                }

                false
            }
        };

        Ok(TestResult::Bool(result ^ test.is_not))
    }

    fn find_envelopes(
        &self,
        envelope_list: &[Envelope],
        address_part: AddressPart,
        zone: Option<i64>,
        mut cb: impl FnMut(&str) -> bool,
    ) -> bool {
        for (name, value) in &self.envelope {
            if envelope_list.contains(name)
                && match (name, zone) {
                    (Envelope::From | Envelope::To | Envelope::Orcpt, _) => {
                        if let Some(value) = address_part.eval_string(value.to_string().as_ref()) {
                            cb(value)
                        } else {
                            false
                        }
                    }
                    (Envelope::ByTimeAbsolute, Some(zone)) => {
                        if let Some(dt) = DateTime::parse_rfc3339(value.to_string().as_ref()) {
                            cb(&dt.to_timezone(zone).to_rfc3339())
                        } else {
                            cb("")
                        }
                    }
                    _ => cb(value.to_string().as_ref()),
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
        std::str::from_utf8(&addr[addr_start_pos..addr_end_pos]).ok()
    } else {
        match addr.get(addr_start_pos..addr_end_pos) {
            Some(addr) if at_pos == 0 && addr.eq_ignore_ascii_case(b"mailer-daemon") => {
                std::str::from_utf8(addr).ok()
            }
            _ => None,
        }
    }
}
