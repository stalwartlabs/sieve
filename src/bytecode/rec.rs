/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::REC_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Rec {
    pub tag: u8,
    pub b: u8,
    pub c: u16,
    pub d: u32,
    pub e: u64,
}

impl Rec {
    pub(crate) const fn tagged(tag: u8) -> Self {
        Rec {
            tag,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
        }
    }

    pub(crate) fn encode(self) -> [u8; REC_LEN] {
        let mut out = [0u8; REC_LEN];
        out[0] = self.tag;
        out[1] = self.b;
        out[2..4].copy_from_slice(&self.c.to_le_bytes());
        out[4..8].copy_from_slice(&self.d.to_le_bytes());
        out[8..16].copy_from_slice(&self.e.to_le_bytes());
        out
    }

    pub(crate) fn decode(bytes: &[u8; REC_LEN]) -> Self {
        Rec {
            tag: bytes[0],
            b: bytes[1],
            c: u16::from_le_bytes([bytes[2], bytes[3]]),
            d: u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            e: u64::from_le_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ]),
        }
    }

    pub(crate) fn str(self) -> Str {
        Str {
            off: self.d,
            len: self.e as u32,
        }
    }

    pub(crate) fn with_str(mut self, s: Str) -> Self {
        self.d = s.off;
        self.e = s.len as u64;
        self
    }

    pub(crate) fn range(self) -> Range {
        Range {
            start: self.d,
            len: self.e as u32,
        }
    }

    pub(crate) fn with_range(mut self, r: Range) -> Self {
        self.d = r.start;
        self.e = r.len as u64;
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Str {
    pub off: u32,
    pub len: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct Range {
    pub start: u32,
    pub len: u32,
}

impl Range {
    pub(crate) const EMPTY: Range = Range { start: 0, len: 0 };

    pub(crate) fn is_empty(self) -> bool {
        self.len == 0
    }
}

pub(crate) mod tag {
    pub const NONE: u8 = 0;
    pub const TEXT: u8 = 1;
    pub const INT: u8 = 2;
    pub const FLOAT: u8 = 3;
    pub const VAR_LOCAL: u8 = 4;
    pub const VAR_MATCH: u8 = 5;
    pub const VAR_GLOBAL: u8 = 6;
    pub const VAR_ENV: u8 = 7;
    pub const VAR_ENVELOPE: u8 = 8;
    pub const VAR_PART: u8 = 9;
    pub const VAR_HEADER: u8 = 10;
    pub const CONT: u8 = 11;
    pub const REGEX: u8 = 12;
    pub const GLOB: u8 = 13;
    pub const HEADER: u8 = 14;
    pub const LIST: u8 = 15;
    pub const REF: u8 = 16;
    pub const BIN_OP: u8 = 20;
    pub const UN_OP: u8 = 21;
    pub const JMP_IF: u8 = 22;
    pub const CALL: u8 = 23;
    pub const ARRAY_ACCESS: u8 = 24;
    pub const ARRAY_BUILD: u8 = 25;
    pub const MODIFIER: u8 = 30;
    pub const CAPABILITY: u8 = 31;
    pub const ENVELOPE: u8 = 32;
    pub const NOTIFY_ITEM: u8 = 33;
    pub const VARIABLE_NONE: u8 = 34;
}
