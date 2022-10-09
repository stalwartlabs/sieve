use mail_parser::{
    parsers::{
        fields::address::{
            parse_address, parse_address_detail_part, parse_address_domain,
            parse_address_local_part, parse_address_user_part,
        },
        message::MessageStream,
    },
    Header, HeaderName, HeaderValue, Message,
};

use crate::{
    compiler::grammar::{tests::test_address::TestAddress, AddressPart, MatchType},
    Context,
};

use super::test_header::MessageHeaders;

impl TestAddress {
    pub(crate) fn exec(&self, ctx: &mut Context, message: &Message) -> bool {
        let key_list = ctx.eval_strings(&self.key_list);
        let header_list = ctx.parse_header_names(&self.header_list);

        (match &self.match_type {
            MatchType::Is | MatchType::Contains => {
                let is_is = matches!(&self.match_type, MatchType::Is);
                message.find_headers(
                    ctx.part,
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header| {
                        message.find_addresses(header, &self.address_part, |value| {
                            for key in &key_list {
                                if is_is {
                                    if self.comparator.is(value, key.as_ref()) {
                                        return true;
                                    }
                                } else if self.comparator.contains(value, key.as_ref()) {
                                    return true;
                                }
                            }
                            false
                        })
                    },
                )
            }
            MatchType::Value(rel_match) => message.find_headers(
                ctx.part,
                &header_list,
                self.index,
                self.mime_anychild,
                |header| {
                    message.find_addresses(header, &self.address_part, |value| {
                        for key in &key_list {
                            if self.comparator.relational(rel_match, value, key.as_ref()) {
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
                let result = message.find_headers(
                    ctx.part,
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header| {
                        message.find_addresses(header, &self.address_part, |value| {
                            for key in &key_list {
                                if is_matches {
                                    if self.comparator.matches(
                                        value,
                                        key.as_ref(),
                                        *capture_positions,
                                        &mut captured_positions,
                                    ) {
                                        return true;
                                    }
                                } else if self.comparator.regex(
                                    value,
                                    key.as_ref(),
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
                let mut count = 0;
                message.find_headers(
                    ctx.part,
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header| {
                        message.find_addresses(header, &self.address_part, |value| {
                            if !value.is_empty() {
                                count += 1;
                            }
                            false
                        })
                    },
                );

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

pub(crate) trait MessageAddresses {
    fn find_addresses(
        &self,
        header: &Header,
        part: &AddressPart,
        visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool;
}

impl<'x> MessageAddresses for Message<'x> {
    #[allow(unused_assignments)]
    fn find_addresses(
        &self,
        header: &Header,
        part: &AddressPart,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        let value_;
        let value = if let HeaderName::Rfc(_) = &header.name {
            value_ = None;
            &header.value
        } else {
            value_ = parse_address(&mut MessageStream::new(
                self.raw_message
                    .get(header.offset_start..header.offset_end + 2)
                    .unwrap_or(b""),
            ))
            .into();
            value_.as_ref().unwrap()
        };

        match value {
            HeaderValue::Address(addr) => {
                if let Some(addr) = addr
                    .address
                    .as_deref()
                    .or(addr.name.as_deref())
                    .and_then(|a| part.eval(a))
                {
                    visitor_fnc(addr)
                } else {
                    false
                }
            }
            HeaderValue::AddressList(addr_list) => {
                for addr in addr_list {
                    if let Some(addr) = addr
                        .address
                        .as_deref()
                        .or(addr.name.as_deref())
                        .and_then(|a| part.eval(a))
                    {
                        if visitor_fnc(addr) {
                            return true;
                        }
                    }
                }
                false
            }
            HeaderValue::Group(group) => {
                for addr in &group.addresses {
                    if let Some(addr) = addr
                        .address
                        .as_deref()
                        .or(addr.name.as_deref())
                        .and_then(|a| part.eval(a))
                    {
                        if visitor_fnc(addr) {
                            return true;
                        }
                    }
                }
                false
            }
            HeaderValue::GroupList(group_list) => {
                for group in group_list {
                    for addr in &group.addresses {
                        if let Some(addr) = addr
                            .address
                            .as_deref()
                            .or(addr.name.as_deref())
                            .and_then(|a| part.eval(a))
                        {
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

impl AddressPart {
    pub(crate) fn eval<'x>(&self, addr: &'x str) -> Option<&'x str> {
        match self {
            AddressPart::All => addr.into(),
            AddressPart::LocalPart => parse_address_local_part(addr),
            AddressPart::Domain => parse_address_domain(addr),
            AddressPart::User => parse_address_user_part(addr),
            AddressPart::Detail => parse_address_detail_part(addr),
        }
    }
}
