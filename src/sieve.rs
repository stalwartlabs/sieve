/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{
    bytecode::{
        Corrupt, Decoded, FORMAT_VERSION, HEADER_LEN, REC_LEN, Sections,
        header_id::{HEADER_OTHER, header_from_id},
        rec::{Range, Rec, Str},
        verify::verify,
    },
    runtime::tests::glob::GlobView,
};
use mail_parser::HeaderName;
use std::{
    borrow::Cow,
    cell::UnsafeCell,
    fmt::{Debug, Display, Formatter},
    sync::OnceLock,
};

pub struct Sieve<'a> {
    code: Cow<'a, [u8]>,
    records: Cow<'a, [u8]>,
    blob: Cow<'a, str>,
    header_names_raw: Cow<'a, [u8]>,
    globs: Cow<'a, [u8]>,
    header_names: Box<[HeaderName<'static>]>,
    regexes: Box<[OnceLock<Option<fancy_regex::Regex>>]>,
    num_vars: u16,
    num_match_vars: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoadError {
    Truncated,
    UnsupportedVersion(u16),
    Corrupted,
}

impl From<Corrupt> for LoadError {
    fn from(_: Corrupt) -> Self {
        LoadError::Corrupted
    }
}

impl Display for LoadError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Truncated => f.write_str("Truncated Sieve script"),
            LoadError::UnsupportedVersion(version) => write!(
                f,
                "Sieve script was compiled with format version {version}, expected {FORMAT_VERSION}"
            ),
            LoadError::Corrupted => f.write_str("Corrupted Sieve script"),
        }
    }
}

impl std::error::Error for LoadError {}

pub(crate) struct RecIter<'s> {
    bytes: &'s [u8],
    pub(crate) index: u32,
}

impl Iterator for RecIter<'_> {
    type Item = Rec;

    #[inline(always)]
    fn next(&mut self) -> Option<Rec> {
        let (chunk, rest) = self.bytes.split_first_chunk::<REC_LEN>()?;
        self.bytes = rest;
        self.index += 1;
        Some(Rec::decode(chunk))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.bytes.len() / REC_LEN;
        (len, Some(len))
    }
}

impl ExactSizeIterator for RecIter<'_> {}

impl<'a> Sieve<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Result<Sieve<'a>, LoadError> {
        let sections = Self::sections(bytes)?;
        let blob = std::str::from_utf8(&bytes[sections.blob.0..sections.blob.1])
            .map_err(|_| LoadError::Corrupted)?;
        let sieve = Self::build(bytes, sections, Cow::Borrowed(blob))?;
        verify(&sieve)?;
        Ok(sieve)
    }

    #[allow(clippy::missing_safety_doc)]
    pub unsafe fn from_bytes_unchecked(bytes: &'a [u8]) -> Result<Sieve<'a>, LoadError> {
        let sections = Self::sections(bytes)?;
        let blob =
            unsafe { std::str::from_utf8_unchecked(&bytes[sections.blob.0..sections.blob.1]) };
        Self::build(bytes, sections, Cow::Borrowed(blob))
    }

    fn sections(bytes: &[u8]) -> Result<Sections, LoadError> {
        Sections::parse(bytes)
    }

    fn build(
        bytes: &'a [u8],
        sections: Sections,
        blob: Cow<'a, str>,
    ) -> Result<Sieve<'a>, LoadError> {
        let header_names_raw = &bytes[sections.header_names.0..sections.header_names.1];
        let header_names = parse_header_names(header_names_raw, &blob)?;
        Ok(Sieve {
            code: Cow::Borrowed(&bytes[sections.code.0..sections.code.1]),
            records: Cow::Borrowed(&bytes[sections.records.0..sections.records.1]),
            blob,
            header_names_raw: Cow::Borrowed(header_names_raw),
            globs: Cow::Borrowed(&bytes[sections.globs.0..sections.globs.1]),
            header_names,
            regexes: new_regex_cache(sections.num_regexes),
            num_vars: sections.num_vars,
            num_match_vars: sections.num_match_vars,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn from_parts(
        code: Vec<u8>,
        records: Vec<u8>,
        blob: String,
        header_names_raw: Vec<u8>,
        globs: Vec<u8>,
        num_regexes: u32,
        num_vars: u16,
        num_match_vars: u16,
    ) -> Result<Sieve<'static>, LoadError> {
        let header_names = parse_header_names(&header_names_raw, &blob)?;
        Ok(Sieve {
            code: Cow::Owned(code),
            records: Cow::Owned(records),
            blob: Cow::Owned(blob),
            header_names_raw: Cow::Owned(header_names_raw),
            globs: Cow::Owned(globs),
            header_names,
            regexes: new_regex_cache(num_regexes),
            num_vars,
            num_match_vars,
        })
    }

    pub fn into_owned(self) -> Sieve<'static> {
        Sieve {
            code: Cow::Owned(self.code.into_owned()),
            records: Cow::Owned(self.records.into_owned()),
            blob: Cow::Owned(self.blob.into_owned()),
            header_names_raw: Cow::Owned(self.header_names_raw.into_owned()),
            globs: Cow::Owned(self.globs.into_owned()),
            header_names: self.header_names,
            regexes: self.regexes,
            num_vars: self.num_vars,
            num_match_vars: self.num_match_vars,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(self.serialized_len());
        let mut start = HEADER_LEN;
        let mut section = |len: usize| {
            let range = (start, start + len);
            start += len;
            range
        };
        Sections {
            num_vars: self.num_vars,
            num_match_vars: self.num_match_vars,
            code: section(self.code.len()),
            records: section(self.records.len()),
            blob: section(self.blob.len()),
            header_names: section(self.header_names_raw.len()),
            globs: section(self.globs.len()),
            num_regexes: self.regexes.len() as u32,
        }
        .write_header(&mut out);
        out.extend_from_slice(&self.code);
        out.extend_from_slice(&self.records);
        out.extend_from_slice(self.blob.as_bytes());
        out.extend_from_slice(&self.header_names_raw);
        out.extend_from_slice(&self.globs);
        out
    }

    pub fn serialized_len(&self) -> usize {
        HEADER_LEN
            + self.code.len()
            + self.records.len()
            + self.blob.len()
            + self.header_names_raw.len()
            + self.globs.len()
    }

    pub fn code_len(&self) -> usize {
        self.code.len()
    }

    pub fn constant_count(&self) -> usize {
        self.blob.len()
    }

    #[inline(always)]
    pub(crate) fn code(&self) -> &[u8] {
        &self.code
    }

    #[inline(always)]
    pub(crate) fn num_vars(&self) -> usize {
        self.num_vars as usize
    }

    #[inline(always)]
    pub(crate) fn num_match_vars(&self) -> usize {
        self.num_match_vars as usize
    }

    #[inline(always)]
    pub(crate) fn num_records(&self) -> u32 {
        (self.records.len() / REC_LEN) as u32
    }

    #[inline(always)]
    pub(crate) fn num_globs(&self) -> u32 {
        self.globs
            .first_chunk::<4>()
            .map_or(0, |b| u32::from_le_bytes(*b))
    }

    #[inline(always)]
    pub(crate) fn num_regexes(&self) -> u32 {
        self.regexes.len() as u32
    }

    #[inline(always)]
    pub(crate) fn rec(&self, index: u32) -> Decoded<Rec> {
        let start = index as usize * REC_LEN;
        self.records
            .get(start..)
            .and_then(|s| s.first_chunk::<REC_LEN>())
            .map(Rec::decode)
            .ok_or(Corrupt)
    }

    #[inline(always)]
    pub(crate) fn recs(&self, range: Range) -> Decoded<RecIter<'_>> {
        let start = range.start as usize * REC_LEN;
        let len = range.len as usize * REC_LEN;
        self.records
            .get(start..start.checked_add(len).ok_or(Corrupt)?)
            .map(|bytes| RecIter {
                bytes,
                index: range.start,
            })
            .ok_or(Corrupt)
    }

    #[inline(always)]
    pub(crate) fn str(&self, s: Str) -> Decoded<&str> {
        let start = s.off as usize;
        self.blob
            .get(start..start.checked_add(s.len as usize).ok_or(Corrupt)?)
            .ok_or(Corrupt)
    }

    #[inline(always)]
    pub(crate) fn header_name(&self, index: u16) -> Decoded<&HeaderName<'static>> {
        self.header_names.get(index as usize).ok_or(Corrupt)
    }

    pub(crate) fn glob(&self, index: u16) -> Decoded<GlobView<'_>> {
        let count = self.num_globs();
        if index as u32 >= count {
            return Err(Corrupt);
        }
        let at = 4 + index as usize * 4;
        let offset = self
            .globs
            .get(at..at + 4)
            .and_then(|b| b.try_into().ok())
            .map(u32::from_le_bytes)
            .ok_or(Corrupt)? as usize;
        GlobView::parse(self.globs.get(offset..).ok_or(Corrupt)?, self)
    }

    pub(crate) fn regex(&self, slot: u16, pattern: &str) -> Option<&fancy_regex::Regex> {
        self.regexes
            .get(slot as usize)?
            .get_or_init(|| fancy_regex::Regex::new(pattern).ok())
            .as_ref()
    }
}

fn new_regex_cache(count: u32) -> Box<[OnceLock<Option<fancy_regex::Regex>>]> {
    (0..count).map(|_| OnceLock::new()).collect()
}

fn parse_header_names(raw: &[u8], blob: &str) -> Result<Box<[HeaderName<'static>]>, LoadError> {
    let Some((count, mut rest)) = raw.split_first_chunk::<4>() else {
        return if raw.is_empty() {
            Ok(Box::default())
        } else {
            Err(LoadError::Corrupted)
        };
    };
    let count = u32::from_le_bytes(*count) as usize;
    if count > rest.len() {
        return Err(LoadError::Corrupted);
    }
    let mut names = Vec::with_capacity(count);
    for _ in 0..count {
        let (&id, tail) = rest.split_first().ok_or(LoadError::Corrupted)?;
        rest = tail;
        if id != HEADER_OTHER {
            names.push(header_from_id(id).ok_or(LoadError::Corrupted)?);
            continue;
        }
        let (entry, tail) = rest.split_first_chunk::<8>().ok_or(LoadError::Corrupted)?;
        rest = tail;
        let off = u32::from_le_bytes([entry[0], entry[1], entry[2], entry[3]]) as usize;
        let len = u32::from_le_bytes([entry[4], entry[5], entry[6], entry[7]]) as usize;
        let name = blob
            .get(off..off.checked_add(len).ok_or(LoadError::Corrupted)?)
            .ok_or(LoadError::Corrupted)?;
        let name = HeaderName::parse(name)
            .map(HeaderName::into_owned)
            .unwrap_or_else(|| HeaderName::Other(Cow::Owned(name.to_string())));
        names.push(name);
    }
    if rest.is_empty() {
        Ok(names.into_boxed_slice())
    } else {
        Err(LoadError::Corrupted)
    }
}

#[derive(Default)]
pub struct ScriptArena {
    #[allow(clippy::vec_box)]
    scripts: UnsafeCell<Vec<Box<Sieve<'static>>>>,
}

impl ScriptArena {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&self, script: Sieve<'static>) -> &Sieve<'static> {
        let boxed = Box::new(script);
        let stable: *const Sieve<'static> = &*boxed;
        unsafe { (*self.scripts.get()).push(boxed) };
        unsafe { &*stable }
    }

    pub fn len(&self) -> usize {
        unsafe { (*self.scripts.get()).len() }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

impl Clone for Sieve<'_> {
    fn clone(&self) -> Self {
        Sieve {
            code: self.code.clone(),
            records: self.records.clone(),
            blob: self.blob.clone(),
            header_names_raw: self.header_names_raw.clone(),
            globs: self.globs.clone(),
            header_names: self.header_names.clone(),
            regexes: self
                .regexes
                .iter()
                .map(|slot| {
                    let cell = OnceLock::new();
                    if let Some(value) = slot.get() {
                        let _ = cell.set(value.clone());
                    }
                    cell
                })
                .collect(),
            num_vars: self.num_vars,
            num_match_vars: self.num_match_vars,
        }
    }
}

impl PartialEq for Sieve<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.code == other.code
            && self.records == other.records
            && self.blob == other.blob
            && self.header_names_raw == other.header_names_raw
            && self.globs == other.globs
            && self.regexes.len() == other.regexes.len()
            && self.num_vars == other.num_vars
            && self.num_match_vars == other.num_match_vars
    }
}

impl Eq for Sieve<'_> {}

impl Debug for Sieve<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Sieve")
            .field("code_len", &self.code.len())
            .field("records", &self.num_records())
            .field("blob_len", &self.blob.len())
            .field("header_names", &self.header_names)
            .field("globs", &self.num_globs())
            .field("regexes", &self.regexes.len())
            .field("num_vars", &self.num_vars)
            .field("num_match_vars", &self.num_match_vars)
            .finish()
    }
}
