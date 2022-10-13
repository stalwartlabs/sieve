use mail_parser::{
    parsers::{
        fields::address::{
            parse_address_detail_part, parse_address_domain, parse_address_local_part,
            parse_address_user_part,
        },
        MessageStream,
    },
    Header, HeaderName, HeaderValue,
};

use crate::{
    compiler::grammar::{tests::test_address::TestAddress, AddressPart, MatchType},
    Context,
};

impl TestAddress {
    pub(crate) fn exec(&self, ctx: &mut Context) -> bool {
        let key_list = ctx.eval_strings(&self.key_list);
        let header_list = ctx.parse_header_names(&self.header_list);

        (match &self.match_type {
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
            MatchType::Value(rel_match) => ctx.find_headers(
                &header_list,
                self.index,
                self.mime_anychild,
                |header, _, _| {
                    ctx.find_addresses(header, &self.address_part, |value| {
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
                let result = ctx.find_headers(
                    &header_list,
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        ctx.find_addresses(header, &self.address_part, |value| {
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
    #[allow(unused_assignments)]
    pub(crate) fn find_addresses(
        &self,
        header: &Header,
        part: &AddressPart,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        let mut raw_header = None;
        let value_;
        let value = if let HeaderName::Rfc(_) = &header.name {
            value_ = None;
            &header.value
        } else {
            let bytes = if header.offset_end > 0 {
                self.message
                    .raw_message
                    .get(header.offset_start..header.offset_end)
                    .unwrap_or(b"")
            } else if let HeaderValue::Text(text) = &header.value {
                // Inserted header
                raw_header = format!("{}\n", text).into_bytes().into();
                raw_header.as_deref().unwrap()
            } else {
                b""
            };

            value_ = MessageStream::new(bytes).parse_address().into();
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
        if !addr.is_empty() {
            match self {
                AddressPart::All => addr.into(),
                AddressPart::LocalPart => parse_address_local_part(addr),
                AddressPart::Domain => parse_address_domain(addr),
                AddressPart::User => parse_address_user_part(addr),
                AddressPart::Detail => parse_address_detail_part(addr),
            }
        } else {
            addr.into()
        }
    }
}
