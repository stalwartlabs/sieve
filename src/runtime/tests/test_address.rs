/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::TestResult;
use crate::{
    Context, Sieve,
    bytecode::ops,
    compiler::{
        Number,
        grammar::{AddressPart, Comparator, MatchType},
    },
    runtime::{RuntimeError, handler::Handler},
};
use mail_parser::{
    Addr, Address, Header, HeaderValue,
    parsers::{
        MessageStream,
        fields::address::{
            parse_address_detail_part, parse_address_domain, parse_address_local_part,
            parse_address_user_part,
        },
    },
};
use smallvec::SmallVec;

impl<'x> Context<'x> {
    pub(crate) fn test_address<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestAddress,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let key_list = self.eval_keys(script, test.key_list)?;
        let header_list = self.parse_header_names(script, test.header_list)?;
        let address_part = AddressPart::from_code(test.address_part);
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();

        let result = match &match_type {
            MatchType::Is | MatchType::Contains | MatchType::Value(_) => self.find_headers(
                &header_list,
                test.index,
                test.mime_anychild,
                |header, _, _| {
                    self.find_addresses(header, &address_part, |value| {
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
                },
            ),
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_values = Vec::new();
                let is_matches = matches!(&match_type, MatchType::Matches(_));
                let to_lower = comparator.is_casemap();
                let mut error = None;
                let result = self.find_headers(
                    &header_list,
                    test.index,
                    test.mime_anychild,
                    |header, _, _| {
                        self.find_addresses(header, &address_part, |value| {
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
                        })
                    },
                );
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
                self.find_headers(
                    &header_list,
                    test.index,
                    test.mime_anychild,
                    |header, _, _| {
                        self.find_addresses(header, &address_part, |value| {
                            if !value.is_empty() {
                                count += 1;
                            }
                            false
                        })
                    },
                );

                key_list
                    .iter()
                    .any(|key| rel_match.cmp(&Number::from(count), &key.value.to_number()))
            }
            MatchType::List => {
                let mut values: Vec<&str> = Vec::new();
                self.find_headers(
                    &header_list,
                    test.index,
                    test.mime_anychild,
                    |header, _, _| {
                        self.find_addresses(header, &address_part, |value| {
                            if !value.is_empty() && !values.contains(&value) {
                                values.push(self.alloc_str(value));
                            }
                            false
                        })
                    },
                );

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

    pub(crate) fn find_addresses(
        &self,
        header: &Header<'_>,
        part: &AddressPart,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        match &header.value {
            HeaderValue::Address(address) => visit_addresses(address, part, &mut visitor_fnc),
            _ => {
                let inserted_header;
                let bytes: &[u8] = if header.offset_end > 0 {
                    self.message
                        .raw_message
                        .get(header.offset_start as usize..header.offset_end as usize)
                        .unwrap_or(b"")
                } else if let HeaderValue::Text(text) = &header.value {
                    inserted_header = format!("{text}\n").into_bytes();
                    &inserted_header
                } else {
                    b""
                };

                match MessageStream::new(bytes).parse_address() {
                    HeaderValue::Address(address) => {
                        visit_addresses(&address, part, &mut visitor_fnc)
                    }
                    _ => visitor_fnc(""),
                }
            }
        }
    }
}

fn visit_addresses(
    address: &Address<'_>,
    part: &AddressPart,
    visitor_fnc: &mut impl FnMut(&str) -> bool,
) -> bool {
    match address {
        Address::List(addr_list) => addr_list
            .iter()
            .any(|addr| part.eval(addr).is_some_and(&mut *visitor_fnc)),
        Address::Group(group_list) => group_list.iter().any(|group| {
            group
                .addresses
                .iter()
                .any(|addr| part.eval(addr).is_some_and(&mut *visitor_fnc))
        }),
    }
}

impl AddressPart {
    pub(crate) fn eval<'x>(&self, addr: &'x Addr<'x>) -> Option<&'x str> {
        let email = addr.address.as_deref().or(addr.name.as_deref());
        match (self, email) {
            (AddressPart::All, _) => email,
            (AddressPart::LocalPart, Some(email)) if !email.is_empty() => {
                parse_address_local_part(email)
            }
            (AddressPart::Domain, Some(email)) if !email.is_empty() => parse_address_domain(email),
            (AddressPart::User, Some(email)) if !email.is_empty() => parse_address_user_part(email),
            (AddressPart::Detail, Some(email)) if !email.is_empty() => {
                parse_address_detail_part(email)
            }
            (AddressPart::Name, _) => addr.name.as_deref(),
            _ => email,
        }
    }

    pub(crate) fn eval_strict<'x>(&self, addr: &'x Addr<'x>) -> Option<&'x str> {
        match (self, addr.address.as_deref()) {
            (AddressPart::All, Some(email)) => Some(email),
            (AddressPart::LocalPart, Some(email)) if !email.is_empty() => {
                parse_address_local_part(email)
            }
            (AddressPart::Domain, Some(email)) if !email.is_empty() => parse_address_domain(email),
            (AddressPart::User, Some(email)) if !email.is_empty() => parse_address_user_part(email),
            (AddressPart::Detail, Some(email)) if !email.is_empty() => {
                parse_address_detail_part(email)
            }
            (AddressPart::Name, _) => addr.name.as_deref(),
            (_, email) => email,
        }
    }

    pub(crate) fn eval_string<'x>(&self, addr: &'x str) -> Option<&'x str> {
        if !addr.is_empty() {
            match self {
                AddressPart::All => addr.into(),
                AddressPart::LocalPart => parse_address_local_part(addr),
                AddressPart::Domain => parse_address_domain(addr),
                AddressPart::User => parse_address_user_part(addr),
                AddressPart::Detail => parse_address_detail_part(addr),
                _ => addr.into(),
            }
        } else {
            addr.into()
        }
    }
}
