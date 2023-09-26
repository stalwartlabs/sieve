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

use mail_parser::{
    parsers::{
        fields::address::{
            parse_address_detail_part, parse_address_domain, parse_address_local_part,
            parse_address_user_part,
        },
        MessageStream,
    },
    Addr, Address, Header, HeaderValue,
};

use crate::{
    compiler::{
        grammar::{tests::test_address::TestAddress, AddressPart, MatchType},
        Number,
    },
    runtime::Variable,
    Context, Event,
};

use super::TestResult;

impl TestAddress {
    pub(crate) fn exec<C>(&self, ctx: &mut Context<C>) -> TestResult {
        let key_list = ctx.eval_values(&self.key_list);
        let header_list = ctx.parse_header_names(&self.header_list);

        let result = match &self.match_type {
            MatchType::Is | MatchType::Contains => {
                let is_is = matches!(&self.match_type, MatchType::Is);
                ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_addresses(header, &self.address_part, |value| {
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
                    },
                )
            }
            MatchType::Value(rel_match) => ctx.find_headers(
                &header_list,
                self.index,
                self.mime_anychild,
                |header, _, _| {
                    ctx.find_addresses(header, &self.address_part, |value| {
                        for key in &key_list {
                            if self
                                .comparator
                                .relational(rel_match, &Variable::from(value), key)
                            {
                                return true;
                            }
                        }
                        false
                    })
                },
            ),
            MatchType::Matches(capture_positions) | MatchType::Regex(capture_positions) => {
                let mut captured_positions = Vec::new();
                let is_matches = matches!(&self.match_type, MatchType::Matches(_));
                let result = ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_addresses(header, &self.address_part, |value| {
                            for (pattern_expr, pattern) in key_list.iter().zip(self.key_list.iter())
                            {
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
                        })
                    },
                );
                if !captured_positions.is_empty() {
                    ctx.set_match_variables(captured_positions);
                }
                result
            }
            MatchType::Count(rel_match) => {
                let mut count: i64 = 0;
                ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_addresses(header, &self.address_part, |value| {
                            if !value.is_empty() {
                                count += 1;
                            }
                            false
                        })
                    },
                );

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

                ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_addresses(header, &self.address_part, |value| {
                            if !value.is_empty() && !values.iter().any(|v| v.eq(value)) {
                                values.push(value.to_string());
                            }
                            false
                        })
                    },
                );

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

impl<'x, C> Context<'x, C> {
    #[allow(unused_assignments)]
    pub(crate) fn find_addresses(
        &self,
        header: &Header,
        part: &AddressPart,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        match &header.value {
            HeaderValue::Address(Address::List(addr_list)) => {
                for addr in addr_list {
                    if let Some(addr) = part.eval(addr) {
                        if visitor_fnc(addr) {
                            return true;
                        }
                    }
                }
                false
            }
            HeaderValue::Address(Address::Group(group_list)) => {
                for group in group_list {
                    for addr in &group.addresses {
                        if let Some(addr) = part.eval(addr) {
                            if visitor_fnc(addr) {
                                return true;
                            }
                        }
                    }
                }
                false
            }
            _ => {
                let mut raw_header = None;
                let bytes = if header.offset_end > 0 {
                    self.message
                        .raw_message
                        .get(header.offset_start..header.offset_end)
                        .unwrap_or(b"")
                } else if let HeaderValue::Text(text) = &header.value {
                    // Inserted header
                    raw_header = format!("{text}\n").into_bytes().into();
                    raw_header.as_deref().unwrap()
                } else {
                    b""
                };

                match MessageStream::new(bytes).parse_address() {
                    HeaderValue::Address(Address::List(addr_list)) => {
                        for addr in &addr_list {
                            if let Some(addr) = part.eval(addr) {
                                if visitor_fnc(addr) {
                                    return true;
                                }
                            }
                        }
                        false
                    }
                    HeaderValue::Address(Address::Group(group_list)) => {
                        for group in group_list {
                            for addr in &group.addresses {
                                if let Some(addr) = part.eval(addr) {
                                    if visitor_fnc(addr) {
                                        return true;
                                    }
                                }
                            }
                        }
                        false
                    }
                    _ => visitor_fnc(""),
                }
            }
        }
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
