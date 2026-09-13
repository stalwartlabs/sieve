/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use std::borrow::Cow;

use mail_parser::decoders::html::html_to_text;
use sieve::{Context, runtime::Variable};

use super::{transform, with_str};

pub fn fn_trim<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| Variable::borrowed(s.trim()))
    })
}

pub fn fn_trim_end<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| Variable::borrowed(s.trim_end()))
    })
}

pub fn fn_trim_start<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| Variable::borrowed(s.trim_start()))
    })
}

pub fn fn_len<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args.first() {
        Some(Variable::String(s)) => s.len(),
        Some(Variable::Array(a)) => a.len(),
        Some(value) => value.to_string().len(),
        None => 0,
    }
    .into()
}

pub fn fn_to_lowercase<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| {
            if s.bytes().any(|b| !b.is_ascii() || b.is_ascii_uppercase()) {
                Variable::from(s.to_lowercase())
            } else {
                Variable::borrowed(s)
            }
        })
    })
}

pub fn fn_to_uppercase<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| {
            if s.bytes().any(|b| !b.is_ascii() || b.is_ascii_lowercase()) {
                Variable::from(s.to_uppercase())
            } else {
                Variable::borrowed(s)
            }
        })
    })
}

pub fn fn_is_uppercase<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| {
            s.chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_uppercase)
                .into()
        })
    })
}

pub fn fn_is_lowercase<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| {
            s.chars()
                .filter(|c| c.is_alphabetic())
                .all(char::is_lowercase)
                .into()
        })
    })
}

pub fn fn_has_digits<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        transform(value, |s| s.bytes().any(|b| b.is_ascii_digit()).into())
    })
}

fn count_chars<'x>(args: &[Variable<'x>], f: impl Fn(&char) -> bool) -> Variable<'x> {
    args.first()
        .map_or(0, |value| value.to_string().chars().filter(f).count())
        .into()
}

pub fn fn_count_spaces<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    count_chars(args, |c| c.is_whitespace())
}

pub fn fn_count_uppercase<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    count_chars(args, |c| c.is_alphabetic() && c.is_uppercase())
}

pub fn fn_count_lowercase<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    count_chars(args, |c| c.is_alphabetic() && c.is_lowercase())
}

pub fn fn_count_chars<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    count_chars(args, |_| true)
}

pub fn fn_eq_ignore_case<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [a, b, ..] = args else {
        return Variable::empty();
    };
    a.to_string().eq_ignore_ascii_case(&b.to_string()).into()
}

pub fn fn_contains<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [haystack, needle, ..] = args else {
        return Variable::empty();
    };
    match haystack {
        Variable::String(s) => s.contains(needle.to_string().as_ref()),
        Variable::Array(items) => items.contains(needle),
        value => value.to_string().contains(needle.to_string().as_ref()),
    }
    .into()
}

fn contains_lowercase(haystack: &str, needle: &str) -> bool {
    if haystack.is_ascii() && needle.is_ascii() {
        needle.is_empty()
            || haystack
                .as_bytes()
                .windows(needle.len())
                .any(|window| window.eq_ignore_ascii_case(needle.as_bytes()))
    } else {
        haystack.to_lowercase().contains(&needle.to_lowercase())
    }
}

pub fn fn_contains_ignore_case<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [haystack, needle, ..] = args else {
        return Variable::empty();
    };
    let needle = needle.to_string();
    match haystack {
        Variable::String(s) => contains_lowercase(s, &needle),
        Variable::Array(items) => items.iter().any(|item| {
            item.as_str()
                .is_some_and(|s| s.eq_ignore_ascii_case(&needle))
        }),
        value => value.to_string().contains(needle.as_ref()),
    }
    .into()
}

pub fn fn_starts_with<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, prefix, ..] = args else {
        return Variable::empty();
    };
    value
        .to_string()
        .starts_with(prefix.to_string().as_ref())
        .into()
}

pub fn fn_ends_with<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, suffix, ..] = args else {
        return Variable::empty();
    };
    value
        .to_string()
        .ends_with(suffix.to_string().as_ref())
        .into()
}

pub fn fn_lines<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args.first() {
        Some(value @ Variable::String(_)) => with_str(value, |s| {
            s.lines().map(Variable::borrowed).collect::<Vec<_>>().into()
        }),
        Some(value) => value.clone(),
        None => Variable::empty(),
    }
}

fn byte_offset(s: &str, chars: usize) -> usize {
    s.char_indices()
        .nth(chars)
        .map_or(s.len(), |(offset, _)| offset)
}

pub fn fn_substring<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, start, count, ..] = args else {
        return Variable::empty();
    };
    let (start, count) = (start.to_usize(), count.to_usize());
    with_str(value, |s| {
        let (_, tail) = s.split_at(byte_offset(s, start));
        let (head, _) = tail.split_at(byte_offset(tail, count));
        Variable::borrowed(head)
    })
}

pub fn fn_strip_prefix<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, prefix, ..] = args else {
        return Variable::empty();
    };
    let prefix = prefix.to_string();
    transform(value, |s| {
        s.strip_prefix(prefix.as_ref())
            .map_or_else(Variable::empty, Variable::borrowed)
    })
}

pub fn fn_strip_suffix<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, suffix, ..] = args else {
        return Variable::empty();
    };
    let suffix = suffix.to_string();
    transform(value, |s| {
        s.strip_suffix(suffix.as_ref())
            .map_or_else(Variable::empty, Variable::borrowed)
    })
}

pub fn fn_split<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, separator, ..] = args else {
        return Variable::empty();
    };
    let separator = separator.to_string();
    with_str(value, |s| {
        s.split(separator.as_ref())
            .map(Variable::borrowed)
            .collect::<Vec<_>>()
            .into()
    })
}

pub fn fn_rsplit<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, separator, ..] = args else {
        return Variable::empty();
    };
    let separator = separator.to_string();
    with_str(value, |s| {
        s.rsplit(separator.as_ref())
            .map(Variable::borrowed)
            .collect::<Vec<_>>()
            .into()
    })
}

pub fn fn_split_n<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, separator, count, ..] = args else {
        return Variable::empty();
    };
    let separator = separator.to_string();
    let count = count.to_integer() as usize;
    with_str(value, |s| {
        let mut rest = s;
        let mut result = Vec::with_capacity(count.min(s.len()) + 1);
        while result.len() < count {
            let Some((head, tail)) = rest.split_once(separator.as_ref()) else {
                break;
            };
            result.push(Variable::borrowed(head));
            rest = tail;
        }
        result.push(Variable::borrowed(rest));
        result.into()
    })
}

fn pair<'s>(parts: Option<(&'s str, &'s str)>) -> Variable<'s> {
    parts.map_or_else(Variable::empty, |(head, tail)| {
        vec![Variable::borrowed(head), Variable::borrowed(tail)].into()
    })
}

pub fn fn_split_once<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, separator, ..] = args else {
        return Variable::empty();
    };
    let separator = separator.to_string();
    with_str(value, |s| pair(s.split_once(separator.as_ref())))
}

pub fn fn_rsplit_once<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, separator, ..] = args else {
        return Variable::empty();
    };
    let separator = separator.to_string();
    with_str(value, |s| pair(s.rsplit_once(separator.as_ref())))
}

pub fn fn_html_to_text<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    args.first().map_or_else(Variable::empty, |value| {
        Variable::String(Cow::Owned(html_to_text(&value.to_string())))
    })
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use sieve::runtime::Variable;

    use super::*;
    use crate::functions::tests::{call, list, s};

    #[test]
    fn trimming() {
        assert_eq!(call(fn_trim, &[s("  hi  ")]), s("hi"));
        assert_eq!(call(fn_trim_start, &[s("  hi  ")]), s("hi  "));
        assert_eq!(call(fn_trim_end, &[s("  hi  ")]), s("  hi"));
        assert_eq!(call(fn_trim, &[list(&[" a", "b "])]), list(&["a", "b"]));
    }

    #[test]
    fn length_and_case() {
        assert_eq!(call(fn_len, &[s("héllo")]), Variable::Integer(6));
        assert_eq!(call(fn_len, &[list(&["a", "b"])]), Variable::Integer(2));
        assert_eq!(
            call(fn_len, &[Variable::Integer(123)]),
            Variable::Integer(3)
        );
        assert_eq!(call(fn_to_lowercase, &[s("HeLLo")]), s("hello"));
        assert_eq!(call(fn_to_lowercase, &[s("ÀB")]), s("àb"));
        assert_eq!(call(fn_to_uppercase, &[s("hello é")]), s("HELLO É"));
        assert_eq!(call(fn_is_uppercase, &[s("ABC 1")]), Variable::Integer(1));
        assert_eq!(call(fn_is_uppercase, &[s("AbC")]), Variable::Integer(0));
        assert_eq!(call(fn_is_lowercase, &[s("abc!")]), Variable::Integer(1));
        assert_eq!(call(fn_is_lowercase, &[s("aBc")]), Variable::Integer(0));
        assert_eq!(call(fn_has_digits, &[s("ab3")]), Variable::Integer(1));
        assert_eq!(call(fn_has_digits, &[s("abc")]), Variable::Integer(0));
    }

    #[test]
    fn counting() {
        assert_eq!(call(fn_count_spaces, &[s("a b\tc")]), Variable::Integer(2));
        assert_eq!(call(fn_count_uppercase, &[s("AbCÉ")]), Variable::Integer(3));
        assert_eq!(call(fn_count_lowercase, &[s("AbCé")]), Variable::Integer(2));
        assert_eq!(call(fn_count_chars, &[s("héllo")]), Variable::Integer(5));
    }

    #[test]
    fn comparisons() {
        assert_eq!(
            call(fn_eq_ignore_case, &[s("HeLLo"), s("hello")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_eq_ignore_case, &[s("hello"), s("world")]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_contains, &[s("hello world"), s("o w")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_contains, &[list(&["a", "b"]), s("b")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_contains, &[list(&["ab"]), s("a")]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_contains_ignore_case, &[s("Hello World"), s("O w")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_contains_ignore_case, &[s("ÀBC"), s("àb")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_contains_ignore_case, &[s("abc"), s("")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_contains_ignore_case, &[list(&["Foo", "bar"]), s("FOO")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_starts_with, &[s("hello"), s("he")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_ends_with, &[s("hello"), s("lo")]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_ends_with, &[s("hello"), s("he")]),
            Variable::Integer(0)
        );
    }

    #[test]
    fn lines_and_substring() {
        assert_eq!(call(fn_lines, &[s("a\r\nb\nc")]), list(&["a", "b", "c"]));
        assert_eq!(
            call(fn_lines, &[Variable::Integer(4)]),
            Variable::Integer(4)
        );
        assert_eq!(
            call(
                fn_substring,
                &[s("héllo wörld"), Variable::Integer(1), Variable::Integer(4)]
            ),
            s("éllo")
        );
        assert_eq!(
            call(
                fn_substring,
                &[s("abc"), Variable::Integer(2), Variable::Integer(10)]
            ),
            s("c")
        );
        assert_eq!(
            call(
                fn_substring,
                &[s("abc"), Variable::Integer(5), Variable::Integer(1)]
            ),
            s("")
        );
    }

    #[test]
    fn stripping() {
        assert_eq!(call(fn_strip_prefix, &[s("foobar"), s("foo")]), s("bar"));
        assert_eq!(call(fn_strip_prefix, &[s("foobar"), s("bar")]), s(""));
        assert_eq!(call(fn_strip_suffix, &[s("foobar"), s("bar")]), s("foo"));
        assert_eq!(
            call(fn_strip_suffix, &[list(&["ab", "cb", "x"]), s("b")]),
            list(&["a", "c", ""])
        );
    }

    #[test]
    fn splitting() {
        assert_eq!(
            call(fn_split, &[s("a,b,c"), s(",")]),
            list(&["a", "b", "c"])
        );
        assert_eq!(
            call(fn_rsplit, &[s("a,b,c"), s(",")]),
            list(&["c", "b", "a"])
        );
        assert_eq!(
            call(fn_split_n, &[s("a,b,c,d"), s(","), Variable::Integer(2)]),
            list(&["a", "b", "c,d"])
        );
        assert_eq!(
            call(fn_split_n, &[s("a,b"), s(","), Variable::Integer(5)]),
            list(&["a", "b"])
        );
        assert_eq!(
            call(fn_split_n, &[s("a,b"), s(","), Variable::Integer(0)]),
            list(&["a,b"])
        );
        assert_eq!(
            call(fn_split_once, &[s("a=b=c"), s("=")]),
            list(&["a", "b=c"])
        );
        assert_eq!(
            call(fn_rsplit_once, &[s("a=b=c"), s("=")]),
            list(&["a=b", "c"])
        );
        assert_eq!(call(fn_split_once, &[s("abc"), s("=")]), s(""));
    }

    #[test]
    fn split_owned_input() {
        let value = Variable::String(Cow::Owned(String::from("x y")));
        let result = call(fn_split, &[value, s(" ")]);
        assert_eq!(result, list(&["x", "y"]));
    }

    #[test]
    fn html() {
        assert_eq!(
            call(fn_html_to_text, &[s("<p>Hello <b>world</b></p>")]),
            s("Hello world\n")
        );
    }
}
