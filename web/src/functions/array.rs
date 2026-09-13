/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

use sieve::{Context, runtime::Variable};

pub fn fn_count<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args.first() {
        Some(Variable::Array(items)) => items.len(),
        Some(value) if !value.is_empty() => 1,
        _ => 0,
    }
    .into()
}

pub fn fn_sort<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let [value, ascending, ..] = args else {
        return Variable::empty();
    };
    let mut items = value.to_array().into_owned();
    if ascending.to_bool() {
        items.sort_unstable();
    } else {
        items.sort_unstable_by(|a, b| b.cmp(a));
    }
    items.into()
}

pub fn fn_dedup<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    let Some(value) = args.first() else {
        return Variable::empty();
    };
    let items = value.to_array();
    let mut result: Vec<Variable<'x>> = Vec::with_capacity(items.len());
    for item in items.iter() {
        if !result.contains(item) {
            result.push(item.clone());
        }
    }
    result.into()
}

pub fn fn_is_intersect<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args {
        [Variable::Array(a), Variable::Array(b), ..] => a.iter().any(|item| b.contains(item)),
        [Variable::Array(items), item, ..] | [item, Variable::Array(items), ..] => {
            items.contains(item)
        }
        _ => false,
    }
    .into()
}

pub fn fn_winnow<'x>(_: &Context<'x>, args: &[Variable<'x>]) -> Variable<'x> {
    match args.first() {
        Some(Variable::Array(items)) => items
            .iter()
            .filter(|item| !item.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .into(),
        Some(value) => value.clone(),
        None => Variable::empty(),
    }
}

#[cfg(test)]
mod tests {
    use sieve::runtime::Variable;

    use super::*;
    use crate::functions::tests::{call, list, s};

    #[test]
    fn count() {
        assert_eq!(
            call(fn_count, &[list(&["a", "b", "c"])]),
            Variable::Integer(3)
        );
        assert_eq!(call(fn_count, &[s("a")]), Variable::Integer(1));
        assert_eq!(call(fn_count, &[s("")]), Variable::Integer(0));
        assert_eq!(
            call(fn_count, &[Variable::Integer(0)]),
            Variable::Integer(1)
        );
    }

    #[test]
    fn sort() {
        assert_eq!(
            call(fn_sort, &[list(&["b", "c", "a"]), Variable::Integer(1)]),
            list(&["a", "b", "c"])
        );
        assert_eq!(
            call(fn_sort, &[list(&["b", "c", "a"]), Variable::Integer(0)]),
            list(&["c", "b", "a"])
        );
        assert_eq!(call(fn_sort, &[s("x"), Variable::Integer(1)]), list(&["x"]));
    }

    #[test]
    fn dedup() {
        assert_eq!(
            call(fn_dedup, &[list(&["a", "b", "a", "c", "b"])]),
            list(&["a", "b", "c"])
        );
        assert_eq!(call(fn_dedup, &[s("")]), list(&[]));
    }

    #[test]
    fn is_intersect() {
        assert_eq!(
            call(fn_is_intersect, &[list(&["a", "b"]), list(&["c", "b"])]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_is_intersect, &[list(&["a", "b"]), list(&["c"])]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_is_intersect, &[s("b"), list(&["a", "b"])]),
            Variable::Integer(1)
        );
        assert_eq!(
            call(fn_is_intersect, &[list(&["a"]), s("b")]),
            Variable::Integer(0)
        );
        assert_eq!(
            call(fn_is_intersect, &[s("a"), s("a")]),
            Variable::Integer(0)
        );
    }

    #[test]
    fn winnow() {
        assert_eq!(
            call(fn_winnow, &[list(&["a", "", "b", ""])]),
            list(&["a", "b"])
        );
        assert_eq!(call(fn_winnow, &[s("x")]), s("x"));
    }
}
