/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::header_id::{HEADER_OTHER, header_id};
use super::{
    cursor::Code,
    rec::{Range, Rec, Str},
};
use crate::{Sieve, runtime::tests::glob::CompiledGlob, sieve::LoadError};
use hashbrown::{HashTable, hash_table::Entry};
use mail_parser::HeaderName;

#[derive(Default)]
pub(crate) struct Emitter {
    pub code: Code,
    records: Vec<u8>,
    blob: String,
    strings: HashTable<Str>,
    header_names: Vec<HeaderEntry>,
    header_map: HashTable<u16>,
    hasher: ahash::RandomState,
    glob_offsets: Vec<u32>,
    globs: Vec<u8>,
    num_regexes: u32,
}

impl Emitter {
    pub(crate) fn new() -> Self {
        Emitter {
            code: Code {
                bytes: Vec::with_capacity(512),
            },
            records: Vec::with_capacity(512),
            blob: String::with_capacity(512),
            strings: HashTable::with_capacity(32),
            header_map: HashTable::with_capacity(8),
            header_names: Vec::with_capacity(8),
            ..Default::default()
        }
    }

    pub(crate) fn str(&mut self, text: &str) -> Str {
        let hash = self.hasher.hash_one(text);
        let blob = &self.blob;
        let hasher = &self.hasher;
        match self.strings.entry(
            hash,
            |s| blob_str(blob, *s) == text,
            |s| hasher.hash_one(blob_str(blob, *s)),
        ) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(entry) => {
                let s = Str {
                    off: self.blob.len() as u32,
                    len: text.len() as u32,
                };
                entry.insert(s);
                self.blob.push_str(text);
                s
            }
        }
    }

    pub(crate) fn num_records(&self) -> u32 {
        (self.records.len() / super::REC_LEN) as u32
    }

    pub(crate) fn push_recs(&mut self, recs: &[Rec]) -> Range {
        let start = self.num_records();
        self.records.reserve(recs.len() * super::REC_LEN);
        for rec in recs {
            self.records.extend_from_slice(&rec.encode());
        }
        Range {
            start,
            len: recs.len() as u32,
        }
    }

    pub(crate) fn header_name(&mut self, name: &HeaderName<'_>) -> u16 {
        let entry = match header_id(name) {
            Some(id) => HeaderEntry::Known(id),
            None => HeaderEntry::Other(self.str(name.as_str())),
        };
        let hash = self.hasher.hash_one(name.as_str());
        let blob = &self.blob;
        let hasher = &self.hasher;
        let header_names = &self.header_names;
        match self.header_map.entry(
            hash,
            |index| header_names.get(*index as usize) == Some(&entry),
            |index| {
                hasher.hash_one(
                    header_names
                        .get(*index as usize)
                        .map_or("", |entry| entry.as_str(blob)),
                )
            },
        ) {
            Entry::Occupied(entry) => *entry.get(),
            Entry::Vacant(slot) => {
                let index = self.header_names.len() as u16;
                slot.insert(index);
                self.header_names.push(entry);
                index
            }
        }
    }

    pub(crate) fn glob(&mut self, glob: &CompiledGlob) -> u16 {
        let index = self.glob_offsets.len() as u16;
        let shape = glob
            .shape()
            .map(|(kind, literal)| (kind, self.str(literal)));
        let mut out = std::mem::take(&mut self.globs);
        self.glob_offsets.push(out.len() as u32);
        out.push(u8::from(glob.to_lower()));
        out.push(u8::from(glob.is_ascii()));
        let (kind, literal) = shape.unwrap_or((0, Str::default()));
        out.push(kind);
        out.extend_from_slice(&literal.off.to_le_bytes());
        out.extend_from_slice(&literal.len.to_le_bytes());
        let count_at = out.len();
        out.extend_from_slice(&0u32.to_le_bytes());
        let mut count = 0u32;
        for ch in glob.encoded_chars() {
            out.extend_from_slice(&ch.to_le_bytes());
            count += 1;
        }
        out[count_at..count_at + 4].copy_from_slice(&count.to_le_bytes());
        self.globs = out;
        index
    }

    pub(crate) fn regex_slot(&mut self) -> u16 {
        let slot = self.num_regexes as u16;
        self.num_regexes += 1;
        slot
    }

    pub(crate) fn finish(
        self,
        num_vars: u16,
        num_match_vars: u16,
    ) -> Result<Sieve<'static>, LoadError> {
        let mut header_names = Vec::with_capacity(4 + self.header_names.len() * 9);
        if !self.header_names.is_empty() {
            header_names.extend_from_slice(&(self.header_names.len() as u32).to_le_bytes());
            for entry in &self.header_names {
                match entry {
                    HeaderEntry::Known(id) => header_names.push(*id),
                    HeaderEntry::Other(s) => {
                        header_names.push(HEADER_OTHER);
                        header_names.extend_from_slice(&s.off.to_le_bytes());
                        header_names.extend_from_slice(&s.len.to_le_bytes());
                    }
                }
            }
        }
        let mut globs = Vec::with_capacity(4 + self.glob_offsets.len() * 4 + self.globs.len());
        if !self.glob_offsets.is_empty() {
            let table_len = 4 + self.glob_offsets.len() as u32 * 4;
            globs.extend_from_slice(&(self.glob_offsets.len() as u32).to_le_bytes());
            for offset in &self.glob_offsets {
                globs.extend_from_slice(&(offset + table_len).to_le_bytes());
            }
            globs.extend_from_slice(&self.globs);
        }
        Sieve::from_parts(
            self.code.bytes,
            self.records,
            self.blob,
            header_names,
            globs,
            self.num_regexes,
            num_vars,
            num_match_vars,
        )
    }
}

fn blob_str(blob: &str, s: Str) -> &str {
    blob.get(s.off as usize..(s.off + s.len) as usize)
        .unwrap_or("")
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderEntry {
    Known(u8),
    Other(Str),
}

impl HeaderEntry {
    fn as_str<'a>(&self, blob: &'a str) -> &'a str {
        match self {
            HeaderEntry::Known(id) => super::header_id::header_from_id(*id)
                .map(|name| name.as_static_str())
                .unwrap_or(""),
            HeaderEntry::Other(s) => blob_str(blob, *s),
        }
    }
}
