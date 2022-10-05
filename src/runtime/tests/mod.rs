use crate::compiler::grammar::{Comparator, MatchType, RelationalMatch};

pub(crate) fn match_value(
    haystack: &str,
    needle: &str,
    match_type: &MatchType,
    comparator: &Comparator,
) -> bool {
    match match_type {
        MatchType::Is => match comparator {
            Comparator::Octet => haystack == needle,
            Comparator::AsciiNumeric => {
                if let (Ok(haystack), Ok(needle)) = (haystack.parse::<i64>(), needle.parse::<i64>())
                {
                    haystack == needle
                } else {
                    false
                }
            }
            _ => haystack.to_lowercase() == needle.to_lowercase(),
        },
        MatchType::Contains => match comparator {
            Comparator::Octet => haystack.contains(needle),
            _ => haystack.to_lowercase().contains(&needle.to_lowercase()),
        },
        MatchType::Matches(vars) => todo!(),
        MatchType::Regex(vars) => todo!(),
        MatchType::Value(value) => match value {
            RelationalMatch::Gt => todo!(),
            RelationalMatch::Ge => todo!(),
            RelationalMatch::Lt => todo!(),
            RelationalMatch::Le => todo!(),
            RelationalMatch::Eq => todo!(),
            RelationalMatch::Ne => todo!(),
        },
        MatchType::Count(count) => todo!(),
    }
}
