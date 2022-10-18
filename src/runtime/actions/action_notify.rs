use mail_parser::{decoders::quoted_printable::HEX_MAP, RfcHeader};

use crate::{
    compiler::grammar::actions::action_notify::Notify, Action, Context, FileCarbonCopy, Importance,
    Mailto, URIScheme, URI,
};

impl Notify {
    pub(crate) fn exec(&self, ctx: &mut Context) {
        if let Some(method) = URI::parse(ctx.eval_string(&self.method).as_ref()) {
            ctx.actions.push(Action::Notify {
                method,
                from: self.from.as_ref().map(|f| ctx.eval_string(f).into_owned()),
                importance: self.importance.as_ref().map_or(Importance::Normal, |i| {
                    match ctx.eval_string(i).as_ref() {
                        "1" => Importance::High,
                        "3" => Importance::Low,
                        _ => Importance::Normal,
                    }
                }),
                options: ctx.eval_strings_owned(&self.options),
                message: self
                    .message
                    .as_ref()
                    .map(|m| ctx.eval_string(m).into_owned()),
                fcc: self.fcc.as_ref().map(|fcc| {
                    Box::new(FileCarbonCopy {
                        mailbox: ctx.eval_string(&fcc.mailbox).into_owned(),
                        mailbox_id: fcc
                            .mailbox_id
                            .as_ref()
                            .map(|m| ctx.eval_string(m).into_owned()),
                        create: fcc.create,
                        flags: ctx.get_local_flags(&fcc.flags),
                        special_use: fcc
                            .special_use
                            .as_ref()
                            .map(|s| ctx.eval_string(s).into_owned()),
                    })
                }),
            });
        }
    }
}

impl URI {
    pub fn parse(uri: &str) -> Option<URI> {
        let (scheme, uri) = uri.split_once(':')?;

        if !uri.is_empty() {
            if scheme.eq_ignore_ascii_case("mailto") {
                URI::parse_mailto(uri)
            } else if scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https") {
                Some(URI::Http {
                    uri: uri.to_string(),
                })
            } else if scheme.eq_ignore_ascii_case("xmpp") {
                Some(URI::Xmpp {
                    uri: uri.to_string(),
                })
            } else if scheme.eq_ignore_ascii_case("tel") {
                Some(URI::Tel {
                    uri: uri.to_string(),
                })
            } else {
                Some(URI::Other {
                    uri: uri.to_string(),
                })
            }
        } else {
            None
        }
    }

    fn parse_mailto(uri: &str) -> Option<URI> {
        enum State {
            Address((RfcHeader, bool)),
            ParamName,
            ParamValue(Mailto),
        }

        let mut params = Vec::new();

        let mut state = State::Address((RfcHeader::To, false));
        let mut buf = Vec::new();
        let uri_ = uri.as_bytes();
        let mut iter = uri_.iter();
        let mut has_addresses = false;

        while let Some(&ch) = iter.next() {
            match ch {
                b'%' => {
                    let hex1 = HEX_MAP[*iter.next()? as usize];
                    let hex2 = HEX_MAP[*iter.next()? as usize];
                    if hex1 != -1 && hex2 != -1 {
                        let ch = ((hex1 as u8) << 4) | hex2 as u8;

                        match &state {
                            State::Address((header, has_at)) if ch == b',' => {
                                if *has_at {
                                    insert_unique(
                                        &mut params,
                                        Mailto::Header(*header),
                                        String::from_utf8(std::mem::take(&mut buf)).ok()?,
                                    );
                                    has_addresses = true;
                                    state = State::Address((*header, false));
                                } else {
                                    return None;
                                }
                            }
                            _ => buf.push(ch),
                        }
                    } else {
                        return None;
                    }
                }
                b',' => match &state {
                    State::Address((header, true)) => {
                        insert_unique(
                            &mut params,
                            Mailto::Header(*header),
                            String::from_utf8(std::mem::take(&mut buf)).ok()?,
                        );
                        state = State::Address((*header, false));
                        has_addresses = true;
                    }
                    State::ParamValue(_) => buf.push(ch),
                    _ => return None,
                },
                b'?' => match &state {
                    State::Address((header, has_at)) if *has_at || buf.is_empty() => {
                        if !buf.is_empty() {
                            insert_unique(
                                &mut params,
                                Mailto::Header(*header),
                                String::from_utf8(std::mem::take(&mut buf)).ok()?,
                            );
                            has_addresses = true;
                        }
                        state = State::ParamName;
                    }
                    State::ParamValue(_) => buf.push(ch),
                    _ => return None,
                },
                b'@' => match &state {
                    State::Address((header, false)) if !buf.is_empty() => {
                        buf.push(ch);
                        state = State::Address((*header, true));
                    }
                    State::ParamName | State::ParamValue(_) => buf.push(ch),
                    _ => return None,
                },
                b'=' => match &state {
                    State::ParamName if !buf.is_empty() => {
                        let param = String::from_utf8(std::mem::take(&mut buf)).ok()?;
                        state = if let Some(header) = RfcHeader::parse(&param) {
                            if matches!(header, RfcHeader::To | RfcHeader::Cc | RfcHeader::Bcc) {
                                State::Address((header, false))
                            } else {
                                State::ParamValue(Mailto::Header(header))
                            }
                        } else if param.eq_ignore_ascii_case("body") {
                            State::ParamValue(Mailto::Body)
                        } else {
                            State::ParamValue(Mailto::Other(param))
                        };
                    }
                    State::ParamValue(_) => buf.push(ch),
                    _ => return None,
                },
                b'&' => match state {
                    State::Address((header, true)) => {
                        if !buf.is_empty() {
                            insert_unique(
                                &mut params,
                                Mailto::Header(header),
                                String::from_utf8(std::mem::take(&mut buf)).ok()?,
                            );
                        }
                        state = State::ParamName;
                    }
                    State::ParamValue(param) => {
                        if !buf.is_empty() {
                            params.push((param, String::from_utf8(std::mem::take(&mut buf)).ok()?));
                        }
                        state = State::ParamName;
                    }
                    _ => return None,
                },
                _ => match &state {
                    State::ParamName => {
                        if ch.is_ascii_alphanumeric() || [b'-', b'_'].contains(&ch) {
                            buf.push(ch);
                        } else {
                            return None;
                        }
                    }
                    _ => {
                        if !ch.is_ascii_whitespace() {
                            buf.push(ch);
                        }
                    }
                },
            }
        }

        if !buf.is_empty() {
            let value = String::from_utf8(std::mem::take(&mut buf)).ok()?;
            match state {
                State::Address((header, true)) => {
                    insert_unique(&mut params, Mailto::Header(header), value);
                    has_addresses = true;
                }
                State::ParamName => {
                    params.push((Mailto::Other(value), String::new()));
                }
                State::ParamValue(param) => {
                    params.push((param, value));
                }
                _ => return None,
            }
        }

        if has_addresses {
            Some(URI::Mailto { params })
        } else {
            None
        }
    }

    pub fn scheme(&self) -> URIScheme {
        match self {
            URI::Mailto { .. } => URIScheme::Mailto,
            URI::Xmpp { .. } => URIScheme::Xmpp,
            URI::Http { .. } => URIScheme::Http,
            URI::Tel { .. } => URIScheme::Tel,
            URI::Other { uri } => {
                URIScheme::Other(uri.split_once(':').unwrap().0.to_ascii_lowercase())
            }
        }
    }
}

#[inline(always)]
fn insert_unique(params: &mut Vec<(Mailto, String)>, name: Mailto, value: String) {
    if !params.iter().any(|(_, v)| v.eq_ignore_ascii_case(&value)) {
        params.push((name, value));
    }
}
