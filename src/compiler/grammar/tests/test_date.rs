/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use mail_parser::HeaderName;

use crate::compiler::{
    grammar::{instruction::CompilerState, Capability, Comparator},
    lexer::{word::Word, StringConstant, Token},
    CompileError, ErrorType, Number, Value,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestDate {
    pub header_name: Value,
    pub key_list: Vec<Value>,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub index: Option<i32>,
    pub zone: Zone,
    pub date_part: DatePart,
    pub mime_anychild: bool,
    pub is_not: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) struct TestCurrentDate {
    pub zone: Option<i64>,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub date_part: DatePart,
    pub key_list: Vec<Value>,
    pub is_not: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum Zone {
    Time(i64),
    Original,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
#[cfg_attr(
    feature = "rkyv",
    derive(rkyv::Serialize, rkyv::Deserialize, rkyv::Archive)
)]
pub(crate) enum DatePart {
    Year,
    Month,
    Day,
    Date,
    Julian,
    Hour,
    Minute,
    Second,
    Time,
    Iso8601,
    Std11,
    Zone,
    Weekday,
}

impl CompilerState<'_> {
    pub(crate) fn parse_test_date(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_name = None;
        let mut key_list;
        let mut index = None;
        let mut index_last = false;
        let mut zone = Zone::Local;
        let mut date_part = None;

        let mut mime = false;
        let mut mime_anychild = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex
                    | Word::List),
                ) => {
                    self.validate_argument(
                        1,
                        match word {
                            Word::Value | Word::Count => Capability::Relational.into(),
                            Word::Regex => Capability::Regex.into(),
                            Word::List => Capability::ExtLists.into(),
                            _ => None,
                        },
                        token_info.line_num,
                        token_info.line_pos,
                    )?;

                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::Index) => {
                    self.validate_argument(
                        3,
                        Capability::Index.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    index = (self.tokens.expect_number(u16::MAX as usize)? as i32).into();
                }
                Token::Tag(Word::Last) => {
                    self.validate_argument(
                        4,
                        Capability::Index.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    index_last = true;
                }
                Token::Tag(Word::Mime) => {
                    self.validate_argument(
                        5,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime = true;
                }
                Token::Tag(Word::AnyChild) => {
                    self.validate_argument(
                        6,
                        Capability::Mime.into(),
                        token_info.line_num,
                        token_info.line_pos,
                    )?;
                    mime_anychild = true;
                }
                Token::Tag(Word::OriginalZone) => {
                    self.validate_argument(7, None, token_info.line_num, token_info.line_pos)?;
                    zone = Zone::Original;
                }
                Token::Tag(Word::Zone) => {
                    self.validate_argument(7, None, token_info.line_num, token_info.line_pos)?;
                    zone = Zone::Time(self.parse_timezone()?);
                }
                _ => {
                    if header_name.is_none() {
                        let header = self.parse_string_token(token_info)?;
                        if let Value::Text(header_name) = &header {
                            if HeaderName::parse(header_name.as_ref()).is_none() {
                                return Err(self
                                    .tokens
                                    .unwrap_next()?
                                    .custom(ErrorType::InvalidHeaderName));
                            }
                        }
                        header_name = header.into();
                    } else if date_part.is_none() {
                        if let Token::StringConstant(string) = &token_info.token {
                            if let Some(date_part_) =
                                lookup_date_part(&string.to_string().to_ascii_lowercase())
                            {
                                date_part = date_part_.into();
                                continue;
                            }
                        }
                        return Err(token_info.expected("valid date part"));
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }

        if !mime && mime_anychild {
            return Err(self.tokens.unwrap_next()?.missing_tag(":mime"));
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::Date(TestDate {
            header_name: header_name.unwrap(),
            key_list,
            date_part: date_part.unwrap(),
            match_type,
            comparator,
            index: if index_last { index.map(|i| -i) } else { index },
            zone,
            mime_anychild,
            is_not: false,
        }))
    }

    pub(crate) fn parse_test_currentdate(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut key_list;
        let mut zone = None;
        let mut date_part = None;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex
                    | Word::List),
                ) => {
                    self.validate_argument(
                        1,
                        match word {
                            Word::Value | Word::Count => Capability::Relational.into(),
                            Word::Regex => Capability::Regex.into(),
                            Word::List => Capability::ExtLists.into(),
                            _ => None,
                        },
                        token_info.line_num,
                        token_info.line_pos,
                    )?;

                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    self.validate_argument(2, None, token_info.line_num, token_info.line_pos)?;
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::Zone) => {
                    self.validate_argument(3, None, token_info.line_num, token_info.line_pos)?;
                    zone = self.parse_timezone()?.into();
                }
                _ => {
                    if date_part.is_none() {
                        if let Token::StringConstant(string) = &token_info.token {
                            if let Some(date_part_) =
                                lookup_date_part(&string.to_string().to_ascii_lowercase())
                            {
                                date_part = date_part_.into();
                                continue;
                            }
                        }
                        return Err(token_info.expected("valid date part"));
                    } else {
                        key_list = self.parse_strings_token(token_info)?;
                        break;
                    }
                }
            }
        }
        self.validate_match(&match_type, &mut key_list)?;

        Ok(Test::CurrentDate(TestCurrentDate {
            key_list,
            date_part: date_part.unwrap(),
            match_type,
            comparator,
            zone,
            is_not: false,
        }))
    }

    pub(crate) fn parse_timezone(&mut self) -> Result<i64, CompileError> {
        let token_info = self.tokens.unwrap_next()?;
        if let Token::StringConstant(value) = &token_info.token {
            let timezone = match value {
                StringConstant::String(value) => value.parse::<i64>().unwrap_or(i64::MAX),
                StringConstant::Number(Number::Integer(n)) => *n,
                StringConstant::Number(Number::Float(n)) => *n as i64,
            };

            return match timezone {
                0..=1400 => Ok((timezone / 100 * 3600) + (timezone % 100 * 60)),
                -1200..=-1 => Ok((timezone / 100 * 3600) - (-timezone % 100 * 60)),
                _ => Err(token_info.expected("invalid timezone")),
            };
        }
        Err(token_info.expected("string containing time zone"))
    }
}

/*
     "year"      => the year, "0000" .. "9999".
     "month"     => the month, "01" .. "12".
     "day"       => the day, "01" .. "31".
     "date"      => the date in "yyyy-mm-dd" format.
     "julian"    => the Modified Julian Day, that is, the date
                    expressed as an integer number of days since
                    00:00 UTC on November 17, 1858 (using the Gregorian
                    calendar).  This corresponds to the regular
                    Julian Day minus 2400000.5.  Sample routines to
                    convert to and from modified Julian dates are
                    given in Appendix A.
     "hour"      => the hour, "00" .. "23".
     "minute"    => the minute, "00" .. "59".
     "second"    => the second, "00" .. "60".
     "time"      => the time in "hh:mm:ss" format.
     "iso8601"   => the date and time in restricted ISO 8601 format.
     "std11"     => the date and time in a format appropriate
                    for use in a Date: header field [RFC2822].
     "zone"      => the time zone in use.  If the user specified a
                    time zone with ":zone", "zone" will
                    contain that value.  If :originalzone is specified
                    this value will be the original zone specified
                    in the date-time value.  If neither argument is
                    specified the value will be the server's default
                    time zone in offset format "+hhmm" or "-hhmm".  An
                    offset of 0 (Zulu) always has a positive sign.
     "weekday"   => the day of the week expressed as an integer between
                    "0" and "6". "0" is Sunday, "1" is Monday, etc.
*/

fn lookup_date_part(input: &str) -> Option<DatePart> {
    hashify::tiny_map!(
        input.as_bytes(),
        "year" => DatePart::Year,
        "month" => DatePart::Month,
        "day" => DatePart::Day,
        "date" => DatePart::Date,
        "julian" => DatePart::Julian,
        "hour" => DatePart::Hour,
        "minute" => DatePart::Minute,
        "second" => DatePart::Second,
        "time" => DatePart::Time,
        "iso8601" => DatePart::Iso8601,
        "std11" => DatePart::Std11,
        "zone" => DatePart::Zone,
        "weekday" => DatePart::Weekday,
    )
}
