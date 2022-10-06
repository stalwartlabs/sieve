use phf::phf_map;
use serde::{Deserialize, Serialize};

use crate::compiler::{
    grammar::{command::CompilerState, Comparator},
    lexer::{string::StringItem, word::Word, Token},
    CompileError,
};

use crate::compiler::grammar::{test::Test, MatchType};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestDate {
    pub header_name: StringItem,
    pub key_list: Vec<StringItem>,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub index: Option<u16>,
    pub index_last: bool,
    pub zone: Zone,
    pub date_part: DatePart,
    pub list: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct TestCurrentDate {
    pub zone: Option<i32>,
    pub match_type: MatchType,
    pub comparator: Comparator,
    pub date_part: DatePart,
    pub key_list: Vec<StringItem>,
    pub list: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum Zone {
    Time(i32),
    Original,
    Local,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl<'x> CompilerState<'x> {
    pub(crate) fn parse_test_date(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let mut header_name = None;
        let key_list;
        let mut index = None;
        let mut index_last = false;
        let mut zone = Zone::Local;
        let mut date_part = None;

        let mut list = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::Index) => {
                    index = (self.tokens.expect_number(u16::MAX as usize)? as u16).into();
                }
                Token::Tag(Word::Last) => {
                    index_last = true;
                }
                Token::Tag(Word::List) => {
                    list = true;
                }
                Token::Tag(Word::OriginalZone) => {
                    zone = Zone::Original;
                }
                Token::Tag(Word::Zone) => {
                    let token_info = self.tokens.unwrap_next()?;
                    if let Token::StringConstant(value) = &token_info.token {
                        if let Ok(value) = std::str::from_utf8(value) {
                            if let Ok(timezone) = value.parse() {
                                zone = Zone::Time(timezone);
                                continue;
                            }
                        }
                    }
                    return Err(token_info.expected("string containing time zone"));
                }
                _ => {
                    if header_name.is_none() {
                        header_name = self.parse_string_token(token_info)?.into();
                    } else if date_part.is_none() {
                        if let Token::StringConstant(string) = &token_info.token {
                            if let Ok(string) = std::str::from_utf8(string) {
                                if let Some(date_part_) =
                                    DATE_PART.get(&string.to_ascii_lowercase())
                                {
                                    date_part = (*date_part_).into();
                                    continue;
                                }
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

        Ok(Test::Date(TestDate {
            header_name: header_name.unwrap(),
            key_list,
            date_part: date_part.unwrap(),
            match_type,
            comparator,
            index,
            index_last,
            zone,
            list,
        }))
    }

    pub(crate) fn parse_test_currentdate(&mut self) -> Result<Test, CompileError> {
        let mut match_type = MatchType::Is;
        let mut comparator = Comparator::AsciiCaseMap;
        let key_list;
        let mut zone = None;
        let mut date_part = None;

        let mut list = false;

        loop {
            let token_info = self.tokens.unwrap_next()?;
            match token_info.token {
                Token::Tag(
                    word @ (Word::Is
                    | Word::Contains
                    | Word::Matches
                    | Word::Value
                    | Word::Count
                    | Word::Regex),
                ) => {
                    match_type = self.parse_match_type(word)?;
                }
                Token::Tag(Word::Comparator) => {
                    comparator = self.parse_comparator()?;
                }
                Token::Tag(Word::List) => {
                    list = true;
                }
                Token::Tag(Word::Zone) => {
                    let token_info = self.tokens.unwrap_next()?;
                    if let Token::StringConstant(value) = &token_info.token {
                        if let Ok(value) = std::str::from_utf8(value) {
                            if let Ok(timezone) = value.parse::<i32>() {
                                zone = timezone.into();
                                continue;
                            }
                        }
                    }
                    return Err(token_info.expected("string containing time zone"));
                }
                _ => {
                    if date_part.is_none() {
                        if let Token::StringConstant(string) = &token_info.token {
                            if let Ok(string) = std::str::from_utf8(string) {
                                if let Some(date_part_) =
                                    DATE_PART.get(&string.to_ascii_lowercase())
                                {
                                    date_part = (*date_part_).into();
                                    continue;
                                }
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

        Ok(Test::CurrentDate(TestCurrentDate {
            key_list,
            date_part: date_part.unwrap(),
            match_type,
            comparator,
            zone,
            list,
        }))
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

static DATE_PART: phf::Map<&'static str, DatePart> = phf_map! {
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
};
