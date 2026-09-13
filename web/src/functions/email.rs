/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use sieve::{Context, runtime::Variable};

use super::transform;

fn is_email(address: &str) -> bool {
    let mut last_ch = 0;
    let mut in_quote = false;
    let mut at_count = 0;
    let mut dot_count = 0;
    let mut lp_len = 0;
    let mut value = 0;

    for ch in address.bytes() {
        match ch {
            b'0'..=b'9'
            | b'a'..=b'z'
            | b'A'..=b'Z'
            | b'!'
            | b'#'
            | b'$'
            | b'%'
            | b'&'
            | b'\''
            | b'*'
            | b'+'
            | b'-'
            | b'/'
            | b'='
            | b'?'
            | b'^'
            | b'_'
            | b'`'
            | b'{'
            | b'|'
            | b'}'
            | b'~'
            | 0x7f..=u8::MAX => {
                value += 1;
            }
            b'.' if !in_quote => {
                if last_ch != b'.' && last_ch != b'@' && value != 0 {
                    value += 1;
                    if at_count == 1 {
                        dot_count += 1;
                    }
                } else {
                    return false;
                }
            }
            b'@' if !in_quote => {
                at_count += 1;
                lp_len = value;
                value = 0;
            }
            b'>' | b':' | b',' | b' ' if in_quote => {
                value += 1;
            }
            b'\"' if !in_quote || last_ch != b'\\' => {
                in_quote = !in_quote;
            }
            b'\\' if in_quote && last_ch != b'\\' => (),
            _ => {
                if !in_quote {
                    return false;
                }
            }
        }

        last_ch = ch;
    }

    at_count == 1 && dot_count > 0 && lp_len > 0 && value > 0
}

pub fn fn_is_email<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first()
        .is_some_and(|value| is_email(&value.to_string()))
        .into()
}

#[derive(Clone, Copy)]
enum EmailPart {
    Local,
    Domain,
    Unknown,
}

pub fn fn_email_part<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, part, ..] = args else {
        return Variable::empty();
    };
    let part = match part.to_string().as_ref() {
        "local" => EmailPart::Local,
        "domain" => EmailPart::Domain,
        _ => EmailPart::Unknown,
    };
    transform(value, |s| {
        s.rsplit_once('@')
            .and_then(|(local, domain)| match part {
                EmailPart::Local => Some(local.trim()),
                EmailPart::Domain => Some(domain.trim()),
                EmailPart::Unknown => None,
            })
            .map_or_else(Variable::empty, Variable::borrowed)
    })
}

#[cfg(test)]
mod tests {
    use sieve::runtime::Variable;

    use super::*;
    use crate::functions::tests::{call, list, s};

    #[test]
    fn email_validation() {
        for valid in [
            "john@example.org",
            "john.doe+tag@mail.example.org",
            "\"john doe\"@example.org",
        ] {
            assert_eq!(
                call(fn_is_email, &[s(valid)]),
                Variable::Integer(1),
                "{valid}"
            );
        }
        for invalid in [
            "john@example",
            "john..doe@example.org",
            "@example.org",
            "john@doe@example.org",
            "john doe@example.org",
            "john@",
        ] {
            assert_eq!(
                call(fn_is_email, &[s(invalid)]),
                Variable::Integer(0),
                "{invalid}"
            );
        }
    }

    #[test]
    fn email_parts() {
        assert_eq!(
            call(fn_email_part, &[s("john@example.org"), s("local")]),
            s("john")
        );
        assert_eq!(
            call(fn_email_part, &[s("\"a@b\"@ example.org "), s("domain")]),
            s("example.org")
        );
        assert_eq!(
            call(fn_email_part, &[s("john@example.org"), s("other")]),
            s("")
        );
        assert_eq!(call(fn_email_part, &[s("john"), s("local")]), s(""));
        assert_eq!(
            call(
                fn_email_part,
                &[list(&["a@one.org", "b@two.org"]), s("domain")]
            ),
            list(&["one.org", "two.org"])
        );
    }
}
