/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use mail_parser::parsers::fields::thread::thread_name;
use sieve::{Context, runtime::Variable};

use super::transform;

pub fn fn_thread_name<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| Variable::borrowed(thread_name(s)))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::functions::tests::{call, list, s};

    #[test]
    fn thread_names() {
        assert_eq!(call(fn_thread_name, &[s("Re: Fwd: Hello")]), s("Hello"));
        assert_eq!(call(fn_thread_name, &[s("Hello")]), s("Hello"));
        assert_eq!(
            call(fn_thread_name, &[list(&["RE: a", "b"])]),
            list(&["a", "b"])
        );
    }
}
