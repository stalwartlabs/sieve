/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use std::borrow::Cow;

use mail_parser::{parsers::MessageStream, DateTime, Header, HeaderValue};

use crate::{
    compiler::{
        grammar::{
            tests::test_date::{DatePart, TestCurrentDate, TestDate, Zone},
            MatchType,
        },
        Number,
    },
    Context, Event,
};

use super::TestResult;

impl TestDate {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let header_name = if let Some(header_name) = ctx.parse_header_name(&self.header_name) {
            header_name
        } else {
            return TestResult::Bool(false ^ self.is_not);
        };

        let result = match &self.match_type {
            MatchType::Count(rel_match) => {
                let mut date_count = 0;
                ctx.find_headers(
                    &[header_name],
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        if ctx.find_dates(header).is_some() {
                            date_count += 1;
                        }
                        false
                    },
                );

                let mut result = false;
                for key in &self.key_list {
                    if rel_match.cmp(&Number::from(date_count), &ctx.eval_value(key).to_number()) {
                        result = true;
                        break;
                    }
                }
                result
            }
            MatchType::List => {
                let mut values = Vec::new();
                ctx.find_headers(
                    &[header_name],
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        if let Some(dt) = ctx.find_dates(header) {
                            let value = self.date_part.eval(self.zone.eval(dt.as_ref()).as_ref());
                            if !value.is_empty() && !values.iter().any(|v: &String| v.eq(&value)) {
                                values.push(value);
                            }
                        }

                        false
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
            _ => {
                let key_list = ctx.eval_values(&self.key_list);
                let mut captured_values = Vec::new();

                let result = ctx.find_headers(
                    &[header_name],
                    self.index,
                    self.mime_anychild,
                    |header, _, _| {
                        if let Some(dt) = ctx.find_dates(header) {
                            let date_part =
                                self.date_part.eval(self.zone.eval(dt.as_ref()).as_ref());
                            for key in &key_list {
                                if match &self.match_type {
                                    MatchType::Is => self.comparator.is(&date_part.as_str(), key),
                                    MatchType::Contains => self
                                        .comparator
                                        .contains(&date_part, key.to_string().as_ref()),
                                    MatchType::Value(rel_match) => self.comparator.relational(
                                        rel_match,
                                        &date_part.as_str(),
                                        key,
                                    ),
                                    MatchType::Matches(capture_positions) => {
                                        self.comparator.matches(
                                            &date_part,
                                            key.to_string().as_ref(),
                                            *capture_positions,
                                            &mut captured_values,
                                        )
                                    }
                                    MatchType::Regex(capture_positions) => self.comparator.matches(
                                        &date_part,
                                        key.to_string().as_ref(),
                                        *capture_positions,
                                        &mut captured_values,
                                    ),
                                    MatchType::Count(_) | MatchType::List => false,
                                } {
                                    return true;
                                }
                            }
                        }

                        false
                    },
                );
                if !captured_values.is_empty() {
                    ctx.set_match_variables(captured_values);
                }
                result
            }
        };

        TestResult::Bool(result ^ self.is_not)
    }
}

impl TestCurrentDate {
    pub(crate) fn exec(&self, ctx: &mut Context) -> TestResult {
        let mut result = false;

        match &self.match_type {
            MatchType::Count(rel_match) => {
                for key in &self.key_list {
                    if rel_match.cmp(&Number::from(1.0), &ctx.eval_value(key).to_number()) {
                        result = true;
                        break;
                    }
                }
            }
            MatchType::List => {
                let value = self.date_part.eval(
                    &(if let Some(zone) = self.zone {
                        DateTime::from_timestamp(ctx.current_time).to_timezone(zone)
                    } else {
                        DateTime::from_timestamp(ctx.current_time)
                    }),
                );
                if !value.is_empty() {
                    return TestResult::Event {
                        event: Event::ListContains {
                            lists: ctx.eval_values_owned(&self.key_list),
                            values: vec![value],
                            match_as: self.comparator.as_match(),
                        },
                        is_not: self.is_not,
                    };
                }
            }
            _ => {
                let mut captured_values = Vec::new();
                let date_part = self.date_part.eval(
                    &(if let Some(zone) = self.zone {
                        DateTime::from_timestamp(ctx.current_time).to_timezone(zone)
                    } else {
                        DateTime::from_timestamp(ctx.current_time)
                    }),
                );

                for key in &self.key_list {
                    let key = ctx.eval_value(key);

                    if match &self.match_type {
                        MatchType::Is => self.comparator.is(&date_part.as_str(), &key),
                        MatchType::Contains => self
                            .comparator
                            .contains(&date_part, key.to_string().as_ref()),
                        MatchType::Value(rel_match) => {
                            self.comparator
                                .relational(rel_match, &date_part.as_str(), &key)
                        }
                        MatchType::Matches(capture_positions) => self.comparator.matches(
                            &date_part,
                            key.to_string().as_ref(),
                            *capture_positions,
                            &mut captured_values,
                        ),
                        MatchType::Regex(capture_positions) => self.comparator.matches(
                            &date_part,
                            key.to_string().as_ref(),
                            *capture_positions,
                            &mut captured_values,
                        ),
                        MatchType::Count(_) | MatchType::List => false,
                    } {
                        result = true;
                        break;
                    }
                }

                if !captured_values.is_empty() {
                    ctx.set_match_variables(captured_values);
                }
            }
        }

        TestResult::Bool(result ^ self.is_not)
    }
}

impl<'x> Context<'x> {
    #[allow(unused_assignments)]
    pub(crate) fn find_dates(&self, header: &'x Header) -> Option<Cow<'x, DateTime>> {
        if let HeaderValue::DateTime(dt) = &header.value {
            if dt.is_valid() {
                return Some(Cow::Borrowed(dt));
            }
        } else if header.offset_end > 0 {
            let bytes = self
                .message
                .raw_message
                .get(header.offset_start as usize..header.offset_end as usize)?;
            if let HeaderValue::DateTime(dt) = MessageStream::new(bytes).parse_date() {
                if dt.is_valid() {
                    return Some(Cow::Owned(dt));
                }
            }
        } else if let HeaderValue::Text(text) = &header.value {
            // Inserted header
            let bytes = format!("{text}\n").into_bytes();
            if let HeaderValue::DateTime(dt) = MessageStream::new(&bytes).parse_date() {
                if dt.is_valid() {
                    return Some(Cow::Owned(dt));
                }
            }
        }
        None
    }
}

impl DatePart {
    fn eval(&self, dt: &DateTime) -> String {
        match self {
            DatePart::Year => format!("{:04}", dt.year),
            DatePart::Month => format!("{:02}", dt.month),
            DatePart::Day => format!("{:02}", dt.day),
            DatePart::Date => format!("{:04}-{:02}-{:02}", dt.year, dt.month, dt.day,),
            DatePart::Julian => ((dt.julian_day() as f64 - 2400000.5) as i64).to_string(),
            DatePart::Hour => format!("{:02}", dt.hour),
            DatePart::Minute => format!("{:02}", dt.minute),
            DatePart::Second => format!("{:02}", dt.second),
            DatePart::Time => format!("{:02}:{:02}:{:02}", dt.hour, dt.minute, dt.second,),
            DatePart::Iso8601 => dt.to_rfc3339(),
            DatePart::Std11 => dt.to_rfc822(),
            DatePart::Zone => format!(
                "{}{:02}{:02}",
                if dt.tz_before_gmt && (dt.tz_hour > 0 || dt.tz_minute > 0) {
                    "-"
                } else {
                    "+"
                },
                dt.tz_hour,
                dt.tz_minute
            ),
            DatePart::Weekday => dt.day_of_week().to_string(),
        }
    }
}

impl Zone {
    pub(crate) fn eval<'x>(&self, dt: &'x DateTime) -> Cow<'x, DateTime> {
        match self {
            Zone::Time(tz) => Cow::Owned(dt.to_timezone(*tz)),
            Zone::Original => Cow::Borrowed(dt),
            Zone::Local => Cow::Owned(DateTime::from_timestamp(dt.to_timestamp())),
        }
    }
}
