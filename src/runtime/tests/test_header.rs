/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{TestResult, mime::SubpartIterator};
use crate::{
    Context, Sieve,
    bytecode::{
        ops::{self, MimeOpts},
        rec::{Range, Rec, tag},
    },
    compiler::{
        Number,
        grammar::{Comparator, MatchType},
    },
    runtime::{RuntimeError, eval::ValueRef, handler::Handler},
};
use mail_parser::{Header, HeaderName, HeaderValue, parsers::MessageStream};
use smallvec::SmallVec;

pub(crate) type HeaderNames<'x> = SmallVec<[HeaderName<'x>; 4]>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum MimeOptsRef<'x> {
    None,
    Type,
    Subtype,
    ContentType,
    Param(SmallVec<[&'x str; 2]>),
}

impl<'x> Context<'x> {
    pub(crate) fn test_header<H: Handler<'x>>(
        &mut self,
        script: &'x Sieve<'x>,
        test: &ops::TestHeader,
        handler: &mut H,
    ) -> Result<TestResult, RuntimeError> {
        let key_list = self.eval_keys(script, test.key_list)?;
        let header_list = self.parse_header_names(script, test.header_list)?;
        let mime_opts = self.mime_opts(script, &test.mime_opts)?;
        let comparator = Comparator::from_code(test.comparator);
        let match_type = test.match_type.match_type();

        let result = match &match_type {
            MatchType::Is | MatchType::Contains | MatchType::Value(_) => self.find_headers(
                &header_list,
                test.index,
                test.mime_anychild,
                |header, _, _| {
                    self.find_header_values(header, &mime_opts, |value| {
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
                        self.find_header_values(header, &mime_opts, |value| {
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
                let mut count = 0;
                self.find_headers(
                    &header_list,
                    test.index,
                    test.mime_anychild,
                    |header, _, _| {
                        match &mime_opts {
                            MimeOptsRef::None => {
                                count += 1;
                            }
                            MimeOptsRef::Type | MimeOptsRef::Subtype | MimeOptsRef::ContentType => {
                                if let HeaderValue::ContentType(_) = &header.value {
                                    count += 1;
                                }
                            }
                            MimeOptsRef::Param(params) => {
                                if let HeaderValue::ContentType(ct) = &header.value
                                    && let Some(attributes) = &ct.attributes
                                {
                                    for attr in attributes {
                                        if params.iter().any(|p| p.eq_ignore_ascii_case(&attr.name))
                                        {
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }

                        false
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
                        self.find_header_values(header, &mime_opts, |value| {
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

    pub(crate) fn mime_opts(
        &self,
        script: &'x Sieve<'x>,
        opts: &MimeOpts,
    ) -> Result<MimeOptsRef<'x>, RuntimeError> {
        Ok(match opts.kind {
            0 => MimeOptsRef::Type,
            1 => MimeOptsRef::Subtype,
            2 => MimeOptsRef::ContentType,
            3 => MimeOptsRef::Param(
                self.eval_strings(script, opts.params)?
                    .into_iter()
                    .collect(),
            ),
            _ => MimeOptsRef::None,
        })
    }

    pub(crate) fn parse_header_names(
        &self,
        script: &'x Sieve<'x>,
        header_names: Range,
    ) -> Result<HeaderNames<'x>, RuntimeError> {
        let mut result = HeaderNames::with_capacity(header_names.len as usize);
        let mut iter = script.recs(header_names)?;
        while let Some(rec) = iter.next() {
            if rec.tag == tag::HEADER {
                result.push(borrow_header_name(script.header_name(rec.c)?));
            } else {
                let value = ValueRef::decode(script, rec, &mut iter)?;
                let value = self.eval_value_ref(script, value)?;
                if let Some(header_name) =
                    self.parse_header_name_str(self.intern_cow(value.into_string()))
                {
                    result.push(header_name);
                }
            }
        }
        Ok(result)
    }

    #[inline(always)]
    pub(crate) fn parse_header_name(
        &self,
        script: &'x Sieve<'x>,
        header_name: Rec,
    ) -> Result<Option<HeaderName<'x>>, RuntimeError> {
        if header_name.tag == tag::HEADER {
            return Ok(Some(borrow_header_name(script.header_name(header_name.c)?)));
        }
        let name = self.eval_str(script, header_name)?;
        Ok(self.parse_header_name_str(name))
    }

    #[inline(always)]
    pub(crate) fn parse_header_name_str(&self, name: &'x str) -> Option<HeaderName<'x>> {
        HeaderName::parse(name)
    }

    pub(crate) fn find_headers<'y>(
        &'y self,
        header_names: &[HeaderName<'_>],
        index: Option<i32>,
        any_child: bool,
        mut visitor_fnc: impl FnMut(&'y Header<'x>, u32, usize) -> bool,
    ) -> bool {
        let parts = [self.part];
        let mut part_iter = SubpartIterator::new(self, &parts, any_child);

        while let Some((part_id, message_part)) = part_iter.next() {
            'outer: for header_name in header_names {
                match index {
                    None => {
                        for (pos, header) in message_part
                            .headers
                            .iter()
                            .enumerate()
                            .filter(|(_, h)| &h.name == header_name)
                        {
                            if visitor_fnc(header, part_id, pos) {
                                return true;
                            }
                        }
                    }
                    Some(index) if index >= 0 => {
                        let mut header_count = 0;

                        for (pos, header) in message_part.headers.iter().enumerate() {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header, part_id, pos) {
                                        return true;
                                    }
                                    continue 'outer;
                                }
                            }
                        }
                    }
                    Some(index) => {
                        let index = -index;
                        let mut header_count = 0;

                        for (pos, header) in message_part.headers.iter().enumerate().rev() {
                            if &header.name == header_name {
                                header_count += 1;
                                if header_count == index {
                                    if visitor_fnc(header, part_id, pos) {
                                        return true;
                                    }
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
        false
    }

    #[allow(unused_assignments)]
    pub(crate) fn find_header_values(
        &self,
        header: &Header<'_>,
        mime_opts: &MimeOptsRef<'_>,
        mut visitor_fnc: impl FnMut(&str) -> bool,
    ) -> bool {
        let mut raw_header = None;
        let mut header_value_ = None;
        let header_value = if header.offset_end != 0 {
            &header.value
        } else {
            let value = if let HeaderValue::Text(text) = &header.value {
                text.as_ref()
            } else {
                #[cfg(test)]
                panic!("Unexpected value.");
                #[cfg(not(test))]
                return false;
            };
            if mime_opts == &MimeOptsRef::None {
                return visitor_fnc(value);
            } else {
                raw_header = format!("{value}\n").into_bytes().into();
                header_value_ = MessageStream::new(raw_header.as_ref().unwrap())
                    .parse_content_type()
                    .into();
                header_value_.as_ref().unwrap()
            }
        };

        match (mime_opts, header_value) {
            (MimeOptsRef::None, HeaderValue::Text(text))
                if matches!(
                    &header.name,
                    HeaderName::Subject
                        | HeaderName::Comments
                        | HeaderName::ContentDescription
                        | HeaderName::ContentLocation
                        | HeaderName::ContentTransferEncoding,
                ) =>
            {
                visitor_fnc(text.as_ref())
            }
            (MimeOptsRef::None, _) => {
                let decoded = MessageStream::new(
                    self.message
                        .raw_message
                        .get(header.offset_start as usize..header.offset_end as usize)
                        .unwrap_or(b""),
                )
                .parse_unstructured();

                match decoded {
                    HeaderValue::Text(text) => visitor_fnc(text.as_ref()),
                    _ => visitor_fnc(""),
                }
            }
            (MimeOptsRef::Type, HeaderValue::ContentType(ct)) => visitor_fnc(ct.c_type.as_ref()),
            (MimeOptsRef::Subtype, HeaderValue::ContentType(ct)) => {
                visitor_fnc(ct.c_subtype.as_deref().unwrap_or(""))
            }
            (MimeOptsRef::ContentType, HeaderValue::ContentType(ct)) => {
                if let Some(sub_type) = &ct.c_subtype {
                    visitor_fnc(&format!("{}/{}", ct.c_type, sub_type))
                } else {
                    visitor_fnc(ct.c_type.as_ref())
                }
            }
            (MimeOptsRef::Param(params), HeaderValue::ContentType(ct)) => {
                if let Some(attributes) = &ct.attributes {
                    for param in params {
                        for attr in attributes {
                            if param.eq_ignore_ascii_case(&attr.name)
                                && visitor_fnc(attr.value.as_ref())
                            {
                                return true;
                            }
                        }
                    }
                }
                visitor_fnc("")
            }
            _ => visitor_fnc(""),
        }
    }
}

pub(crate) fn borrow_header_name<'y>(name: &'y HeaderName<'static>) -> HeaderName<'y> {
    match name {
        HeaderName::Other(name) => HeaderName::Other(std::borrow::Cow::Borrowed(name.as_ref())),
        other => other.clone(),
    }
}
