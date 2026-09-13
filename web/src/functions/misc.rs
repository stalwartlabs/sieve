/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use std::net::IpAddr;

use sieve::{Context, runtime::Variable};

pub fn fn_is_empty<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args.first() {
        Some(Variable::String(s)) => s.is_empty(),
        Some(Variable::Integer(_) | Variable::Float(_)) => false,
        Some(Variable::Array(items)) => items.is_empty(),
        None => true,
    }
    .into()
}

pub fn fn_is_number<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    matches!(
        args.first(),
        Some(Variable::Integer(_) | Variable::Float(_))
    )
    .into()
}

pub fn fn_is_ascii<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args.first() {
        Some(Variable::String(s)) => s.is_ascii(),
        Some(Variable::Array(items)) => items
            .iter()
            .all(|item| item.as_str().is_none_or(str::is_ascii)),
        Some(Variable::Integer(_) | Variable::Float(_)) | None => true,
    }
    .into()
}

fn parse_ip(args: &[Variable<'_>]) -> Option<IpAddr> {
    args.first()
        .and_then(|value| value.to_string().parse::<IpAddr>().ok())
}

pub fn fn_is_ip_addr<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    parse_ip(args).is_some().into()
}

pub fn fn_is_ipv4_addr<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    matches!(parse_ip(args), Some(IpAddr::V4(_))).into()
}

pub fn fn_is_ipv6_addr<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    matches!(parse_ip(args), Some(IpAddr::V6(_))).into()
}

#[cfg(test)]
mod tests {
    use sieve::runtime::Variable;

    use super::*;
    use crate::functions::tests::{call, list, s};

    #[test]
    fn emptiness_and_numbers() {
        assert_eq!(call(fn_is_empty, &[s("")]), Variable::Integer(1));
        assert_eq!(call(fn_is_empty, &[s("a")]), Variable::Integer(0));
        assert_eq!(call(fn_is_empty, &[list(&[])]), Variable::Integer(1));
        assert_eq!(
            call(fn_is_empty, &[Variable::Integer(0)]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_is_number, &[Variable::Float(1.5)]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_is_number, &[Variable::Integer(3)]),
            Variable::Integer(1)
        );
        assert!(matches!(
            call(fn_is_number, &[s("3")]),
            Variable::Integer(0)
        ));
    }

    #[test]
    fn ascii() {
        assert_eq!(call(fn_is_ascii, &[s("hello")]), Variable::Integer(1));
        assert_eq!(call(fn_is_ascii, &[s("héllo")]), Variable::Integer(0));
        assert_eq!(
            call(fn_is_ascii, &[list(&["a", "é"])]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_is_ascii, &[Variable::Integer(9)]),
            Variable::Integer(1)
        );
    }

    #[test]
    fn ip_addresses() {
        assert_eq!(
            call(fn_is_ip_addr, &[s("192.168.1.1")]),
            Variable::Integer(1)
        );
        assert_eq!(call(fn_is_ip_addr, &[s("::1")]), Variable::Integer(1));
        assert_eq!(
            call(fn_is_ip_addr, &[s("example.org")]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_is_ipv4_addr, &[s("10.0.0.1")]),
            Variable::Integer(1)
        );
        assert_eq!(call(fn_is_ipv4_addr, &[s("fe80::1")]), Variable::Integer(0));
        assert_eq!(call(fn_is_ipv6_addr, &[s("fe80::1")]), Variable::Integer(1));
        assert_eq!(
            call(fn_is_ipv6_addr, &[s("10.0.0.1")]),
            Variable::Integer(0)
        );
    }
}
