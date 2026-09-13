/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

pub mod array;
pub mod email;
pub mod header;
pub mod misc;
pub mod text;
pub mod url;

use std::borrow::Cow;

use sieve::{FunctionMap, runtime::Variable};

use self::{array::*, email::*, header::*, misc::*, text::*, url::*};

pub fn register() -> FunctionMap {
    FunctionMap::new()
        .with_function("trim", fn_trim)
        .with_function("trim_start", fn_trim_start)
        .with_function("trim_end", fn_trim_end)
        .with_function("len", fn_len)
        .with_function("count", fn_count)
        .with_function("is_empty", fn_is_empty)
        .with_function("is_number", fn_is_number)
        .with_function("is_ascii", fn_is_ascii)
        .with_function("to_lowercase", fn_to_lowercase)
        .with_function("to_uppercase", fn_to_uppercase)
        .with_function("is_email", fn_is_email)
        .with_function("thread_name", fn_thread_name)
        .with_function("html_to_text", fn_html_to_text)
        .with_function("is_uppercase", fn_is_uppercase)
        .with_function("is_lowercase", fn_is_lowercase)
        .with_function("has_digits", fn_has_digits)
        .with_function("count_spaces", fn_count_spaces)
        .with_function("count_uppercase", fn_count_uppercase)
        .with_function("count_lowercase", fn_count_lowercase)
        .with_function("count_chars", fn_count_chars)
        .with_function("dedup", fn_dedup)
        .with_function("lines", fn_lines)
        .with_function("is_ip_addr", fn_is_ip_addr)
        .with_function("is_ipv4_addr", fn_is_ipv4_addr)
        .with_function("is_ipv6_addr", fn_is_ipv6_addr)
        .with_function("winnow", fn_winnow)
        .with_function_args("sort", fn_sort, 2)
        .with_function_args("email_part", fn_email_part, 2)
        .with_function_args("eq_ignore_case", fn_eq_ignore_case, 2)
        .with_function_args("contains", fn_contains, 2)
        .with_function_args("contains_ignore_case", fn_contains_ignore_case, 2)
        .with_function_args("starts_with", fn_starts_with, 2)
        .with_function_args("ends_with", fn_ends_with, 2)
        .with_function_args("uri_part", fn_uri_part, 2)
        .with_function_args("substring", fn_substring, 3)
        .with_function_args("split", fn_split, 2)
        .with_function_args("rsplit", fn_rsplit, 2)
        .with_function_args("split_once", fn_split_once, 2)
        .with_function_args("rsplit_once", fn_rsplit_once, 2)
        .with_function_args("split_n", fn_split_n, 3)
        .with_function_args("strip_prefix", fn_strip_prefix, 2)
        .with_function_args("strip_suffix", fn_strip_suffix, 2)
        .with_function_args("is_intersect", fn_is_intersect, 2)
}

pub fn with_str<'x>(
    value: &Variable<'x>,
    f: impl for<'s> Fn(&'s str) -> Variable<'s>,
) -> Variable<'x> {
    match value {
        Variable::String(Cow::Borrowed(s)) => f(s),
        value => f(&value.to_string()).into_owned(),
    }
}

pub fn transform<'x>(
    value: &Variable<'x>,
    f: impl for<'s> Fn(&'s str) -> Variable<'s>,
) -> Variable<'x> {
    match value {
        Variable::Array(items) => items
            .iter()
            .map(|item| with_str(item, &f))
            .collect::<Vec<_>>()
            .into(),
        value => with_str(value, f),
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use std::borrow::Cow;

    use sieve::{Arena, Compiler, Function, Runtime, runtime::Variable};

    use super::{register, transform};

    pub fn call(f: Function, args: &[Variable<'static>]) -> Variable<'static> {
        let script = Compiler::new().compile(b"keep;").expect("script compiles");
        let runtime = Runtime::new();
        let mut arena = Arena::new();
        let ctx = runtime.filter(b"Subject: test\r\n\r\nbody\r\n", &script, &mut arena);
        f(&ctx, args).into_owned()
    }

    pub fn s(value: &'static str) -> Variable<'static> {
        Variable::borrowed(value)
    }

    pub fn list(items: &[&'static str]) -> Variable<'static> {
        items
            .iter()
            .copied()
            .map(Variable::borrowed)
            .collect::<Vec<_>>()
            .into()
    }

    #[test]
    fn registers_all_functions() {
        let script = Compiler::new().register_functions(&mut register()).compile(
            br#"require ["vnd.stalwart.expressions", "variables"];
                let "a" "trim(' x ')";
                let "b" "substring('hello', 1, 3)";
                let "c" "split_n('a,b,c', ',', 1)";
                let "d" "uri_part('https://example.org/', 'host')";
                let "e" "is_intersect(['a'], 'a')";
                "#,
        );
        assert!(script.is_ok(), "{script:?}");
    }

    #[test]
    fn transform_borrows_and_maps_arrays() {
        let source = String::from("  a ");
        let value = Variable::borrowed(source.as_str());
        let result = transform(&value, |s| Variable::borrowed(s.trim()));
        assert!(matches!(result, Variable::String(Cow::Borrowed("a"))));

        let owned = Variable::from(String::from(" b "));
        let result = transform(&owned, |s| Variable::borrowed(s.trim()));
        assert!(matches!(&result, Variable::String(Cow::Owned(s)) if s == "b"));

        let array = Variable::from(vec![Variable::borrowed(" c"), Variable::Integer(5)]);
        let result = transform(&array, |s| Variable::borrowed(s.trim()));
        assert_eq!(result, Variable::from(vec![s("c"), s("5")]));
    }
}
