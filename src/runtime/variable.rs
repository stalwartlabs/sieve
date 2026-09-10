/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::compiler::Number;
use std::{borrow::Cow, cmp::Ordering, fmt::Display, hash::Hash};

#[derive(Debug, Clone)]
#[cfg_attr(
    any(test, feature = "serde"),
    derive(serde::Serialize, serde::Deserialize)
)]
pub enum Variable<'x> {
    String(Cow<'x, str>),
    Integer(i64),
    Float(f64),
    Array(Array<'x>),
}

#[derive(Debug, Clone)]
pub enum Array<'x> {
    Borrowed(&'x [Variable<'x>]),
    Owned(Vec<Variable<'x>>),
}

impl<'x> Array<'x> {
    #[inline(always)]
    pub fn as_slice(&self) -> &[Variable<'x>] {
        match self {
            Array::Borrowed(items) => items,
            Array::Owned(items) => items,
        }
    }

    pub fn into_vec(self) -> Vec<Variable<'x>> {
        match self {
            Array::Borrowed(items) => items.to_vec(),
            Array::Owned(items) => items,
        }
    }

    #[inline(always)]
    pub fn is_borrowed(&self) -> bool {
        matches!(self, Array::Borrowed(_))
    }
}

impl<'x> std::ops::Deref for Array<'x> {
    type Target = [Variable<'x>];

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        self.as_slice()
    }
}

impl<'x> From<Vec<Variable<'x>>> for Array<'x> {
    fn from(items: Vec<Variable<'x>>) -> Self {
        Array::Owned(items)
    }
}

impl<'x> From<&'x [Variable<'x>]> for Array<'x> {
    fn from(items: &'x [Variable<'x>]) -> Self {
        Array::Borrowed(items)
    }
}

impl PartialEq for Array<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.as_slice() == other.as_slice()
    }
}

impl Eq for Array<'_> {}

impl PartialOrd for Array<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.as_slice().partial_cmp(other.as_slice())
    }
}

impl Hash for Array<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.as_slice().hash(state)
    }
}

impl Default for Variable<'_> {
    #[inline(always)]
    fn default() -> Self {
        Variable::String(Cow::Borrowed(""))
    }
}

impl<'x> Variable<'x> {
    #[inline(always)]
    pub const fn empty() -> Self {
        Variable::String(Cow::Borrowed(""))
    }

    #[inline(always)]
    pub const fn borrowed(s: &'x str) -> Self {
        Variable::String(Cow::Borrowed(s))
    }

    pub fn to_string(&self) -> Cow<'_, str> {
        match self {
            Variable::String(s) => Cow::Borrowed(s.as_ref()),
            Variable::Integer(n) => Cow::Owned(n.to_string()),
            Variable::Float(n) => Cow::Owned(n.to_string()),
            Variable::Array(l) => Cow::Owned(array_to_string(l)),
        }
    }

    pub fn into_string(self) -> Cow<'x, str> {
        match self {
            Variable::String(s) => s,
            Variable::Integer(n) => Cow::Owned(n.to_string()),
            Variable::Float(n) => Cow::Owned(n.to_string()),
            Variable::Array(l) => Cow::Owned(array_to_string(&l)),
        }
    }

    #[inline(always)]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Variable::String(s) => Some(s.as_ref()),
            _ => None,
        }
    }

    pub fn to_number(&self) -> Number {
        self.to_number_checked()
            .unwrap_or(Number::Float(f64::INFINITY))
    }

    pub fn to_number_checked(&self) -> Option<Number> {
        let s = match self {
            Variable::Integer(n) => return Number::Integer(*n).into(),
            Variable::Float(n) => return Number::Float(*n).into(),
            Variable::String(s) if !s.is_empty() => s.as_ref(),
            _ => return None,
        };

        if !s.contains('.') {
            s.parse::<i64>().map(Number::Integer).ok()
        } else {
            s.parse::<f64>().map(Number::Float).ok()
        }
    }

    pub fn to_integer(&self) -> i64 {
        match self {
            Variable::Integer(n) => *n,
            Variable::Float(n) => *n as i64,
            Variable::String(s) if !s.is_empty() => s.parse::<i64>().unwrap_or(0),
            _ => 0,
        }
    }

    pub fn to_usize(&self) -> usize {
        match self {
            Variable::Integer(n) => *n as usize,
            Variable::Float(n) => *n as usize,
            Variable::String(s) if !s.is_empty() => s.parse::<usize>().unwrap_or(0),
            _ => 0,
        }
    }

    pub fn len(&self) -> usize {
        match self {
            Variable::String(s) => s.len(),
            Variable::Integer(_) | Variable::Float(_) => 2,
            Variable::Array(l) => l.iter().map(|v| v.len() + 2).sum(),
        }
    }

    #[inline(always)]
    pub fn is_empty(&self) -> bool {
        match self {
            Variable::String(s) => s.is_empty(),
            _ => false,
        }
    }

    #[inline(always)]
    pub fn as_array(&self) -> Option<&[Variable<'x>]> {
        match self {
            Variable::Array(l) => Some(l),
            _ => None,
        }
    }

    pub fn into_array(self) -> Array<'x> {
        match self {
            Variable::Array(l) => l,
            v if !v.is_empty() => Array::Owned(vec![v]),
            _ => Array::Borrowed(&[]),
        }
    }

    pub fn to_array(&self) -> Cow<'_, [Variable<'x>]> {
        match self {
            Variable::Array(l) => Cow::Borrowed(l.as_slice()),
            v if !v.is_empty() => Cow::Owned(vec![v.clone()]),
            _ => Cow::Borrowed(&[]),
        }
    }

    pub fn into_string_array(self) -> Vec<String> {
        match self {
            Variable::Array(l) => l.iter().map(|i| i.to_string().into_owned()).collect(),
            v if !v.is_empty() => vec![v.to_string().into_owned()],
            _ => vec![],
        }
    }

    pub fn to_string_array(&self) -> Vec<Cow<'_, str>> {
        match self {
            Variable::Array(l) => l.iter().map(|i| i.to_string()).collect(),
            v if !v.is_empty() => vec![v.to_string()],
            _ => vec![],
        }
    }

    pub fn into_owned(self) -> Variable<'static> {
        match self {
            Variable::String(s) => Variable::String(Cow::Owned(s.into_owned())),
            Variable::Integer(n) => Variable::Integer(n),
            Variable::Float(n) => Variable::Float(n),
            Variable::Array(l) => Variable::Array(Array::Owned(
                l.iter().cloned().map(Variable::into_owned).collect(),
            )),
        }
    }

    #[inline(always)]
    pub fn is_borrowed(&self) -> bool {
        match self {
            Variable::String(s) => matches!(s, Cow::Borrowed(_)),
            Variable::Array(l) => l.is_borrowed(),
            _ => true,
        }
    }

    pub fn to_bool(&self) -> bool {
        match self {
            Variable::Float(f) => *f != 0.0,
            Variable::Integer(n) => *n != 0,
            Variable::String(s) => !s.is_empty(),
            Variable::Array(a) => !a.is_empty(),
        }
    }
}

pub(crate) fn array_to_string(items: &[Variable<'_>]) -> String {
    let mut result = String::with_capacity(items.len() * 10);
    for item in items {
        if !result.is_empty() {
            result.push_str("\r\n");
        }
        match item {
            Variable::String(v) => result.push_str(v),
            Variable::Integer(v) => result.push_str(&v.to_string()),
            Variable::Float(v) => result.push_str(&v.to_string()),
            Variable::Array(_) => {}
        }
    }
    result
}

impl From<String> for Variable<'_> {
    fn from(s: String) -> Self {
        Variable::String(Cow::Owned(s))
    }
}

impl<'x> From<&'x String> for Variable<'x> {
    fn from(s: &'x String) -> Self {
        Variable::String(Cow::Borrowed(s.as_str()))
    }
}

impl<'x> From<&'x str> for Variable<'x> {
    fn from(s: &'x str) -> Self {
        Variable::String(Cow::Borrowed(s))
    }
}

impl<'x> From<Cow<'x, str>> for Variable<'x> {
    fn from(s: Cow<'x, str>) -> Self {
        Variable::String(s)
    }
}

impl<'x> From<Vec<Variable<'x>>> for Variable<'x> {
    fn from(l: Vec<Variable<'x>>) -> Self {
        Variable::Array(Array::Owned(l))
    }
}

impl<'x> From<&'x [Variable<'x>]> for Variable<'x> {
    fn from(l: &'x [Variable<'x>]) -> Self {
        Variable::Array(Array::Borrowed(l))
    }
}

impl From<Number> for Variable<'_> {
    fn from(n: Number) -> Self {
        match n {
            Number::Integer(n) => Variable::Integer(n),
            Number::Float(n) => Variable::Float(n),
        }
    }
}

macro_rules! integer_from {
    ($($ty:ty),*) => {
        $(
            impl From<$ty> for Variable<'_> {
                fn from(n: $ty) -> Self {
                    Variable::Integer(n as i64)
                }
            }
        )*
    };
}

integer_from!(usize, i64, u64, i32, u32);

impl From<f64> for Variable<'_> {
    fn from(n: f64) -> Self {
        Variable::Float(n)
    }
}

impl From<bool> for Variable<'_> {
    fn from(b: bool) -> Self {
        Variable::Integer(i64::from(b))
    }
}

impl Hash for Variable<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Variable::String(s) => s.hash(state),
            Variable::Integer(n) => n.hash(state),
            Variable::Float(n) => n.to_bits().hash(state),
            Variable::Array(l) => l.hash(state),
        }
    }
}

impl PartialEq for Variable<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a == b,
            (Self::Float(a), Self::Float(b)) => a == b,
            (Self::Integer(a), Self::Float(b)) | (Self::Float(b), Self::Integer(a)) => {
                *a as f64 == *b
            }
            (Self::String(a), Self::String(b)) => a == b,
            (Self::String(_), Self::Integer(_) | Self::Float(_)) => &self.parse_number() == other,
            (Self::Integer(_) | Self::Float(_), Self::String(_)) => self == &other.parse_number(),
            (Self::Array(a), Self::Array(b)) => a == b,
            _ => false,
        }
    }
}

impl Eq for Variable<'_> {}

#[allow(clippy::non_canonical_partial_ord_impl)]
impl PartialOrd for Variable<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::Integer(a), Self::Integer(b)) => a.partial_cmp(b),
            (Self::Float(a), Self::Float(b)) => a.partial_cmp(b),
            (Self::Integer(a), Self::Float(b)) => (*a as f64).partial_cmp(b),
            (Self::Float(a), Self::Integer(b)) => a.partial_cmp(&(*b as f64)),
            (Self::String(a), Self::String(b)) => a.partial_cmp(b),
            (Self::String(_), Self::Integer(_) | Self::Float(_)) => {
                self.parse_number().partial_cmp(other)
            }
            (Self::Integer(_) | Self::Float(_), Self::String(_)) => {
                self.partial_cmp(&other.parse_number())
            }
            (Self::Array(a), Self::Array(b)) => a.partial_cmp(b),
            (Self::Array(_) | Self::String(_), _) => Ordering::Greater.into(),
            (_, Self::Array(_)) => Ordering::Less.into(),
        }
    }
}

impl Ord for Variable<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.partial_cmp(other).unwrap_or(Ordering::Greater)
    }
}

impl Display for Variable<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Variable::String(v) => v.fmt(f),
            Variable::Integer(v) => v.fmt(f),
            Variable::Float(v) => v.fmt(f),
            Variable::Array(v) => {
                for (i, v) in v.iter().enumerate() {
                    if i > 0 {
                        f.write_str("\n")?;
                    }
                    v.fmt(f)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(any(test, feature = "serde"))]
impl serde::Serialize for Array<'_> {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_seq(self.as_slice())
    }
}

#[cfg(any(test, feature = "serde"))]
impl<'de, 'x> serde::Deserialize<'de> for Array<'x> {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Vec::<Variable<'x>>::deserialize(deserializer).map(Array::Owned)
    }
}
