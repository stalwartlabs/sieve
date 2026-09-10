/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{
    Corrupt, Decoded, REC_LEN,
    rec::{Range, Rec, Str},
};

pub(crate) struct Cursor<'a> {
    code: &'a [u8],
    pub pos: usize,
    trace: Option<Trace>,
}

#[derive(Default)]
pub(crate) struct Trace {
    pub ranges: Vec<Range>,
    pub strs: Vec<Str>,
    pub recs: Vec<Rec>,
    pub jumps: Vec<u32>,
    pub masks: Vec<u64>,
}

impl<'a> Cursor<'a> {
    pub(crate) fn new(code: &'a [u8], pos: usize) -> Self {
        Cursor {
            code,
            pos,
            trace: None,
        }
    }

    pub(crate) fn tracing(code: &'a [u8]) -> Self {
        Cursor {
            code,
            pos: 0,
            trace: Some(Trace::default()),
        }
    }

    pub(crate) fn trace_mut(&mut self) -> &mut Trace {
        self.trace.get_or_insert_with(Trace::default)
    }

    #[inline(always)]
    pub(crate) fn at_end(&self) -> bool {
        self.pos >= self.code.len()
    }

    #[inline(always)]
    fn take<const N: usize>(&mut self) -> Decoded<&'a [u8; N]> {
        let bytes = self
            .code
            .get(self.pos..)
            .and_then(|s| s.first_chunk::<N>())
            .ok_or(Corrupt)?;
        self.pos += N;
        Ok(bytes)
    }

    #[inline(always)]
    pub(crate) fn u8(&mut self) -> Decoded<u8> {
        self.take::<1>().map(|b| b[0])
    }

    #[inline(always)]
    pub(crate) fn bool(&mut self) -> Decoded<bool> {
        self.u8().map(|b| b != 0)
    }

    #[inline(always)]
    pub(crate) fn u16(&mut self) -> Decoded<u16> {
        self.take::<2>().map(|b| u16::from_le_bytes(*b))
    }

    #[inline(always)]
    pub(crate) fn u32(&mut self) -> Decoded<u32> {
        self.take::<4>().map(|b| u32::from_le_bytes(*b))
    }

    #[inline(always)]
    pub(crate) fn i32(&mut self) -> Decoded<i32> {
        self.take::<4>().map(|b| i32::from_le_bytes(*b))
    }

    #[inline(always)]
    pub(crate) fn u64(&mut self) -> Decoded<u64> {
        self.take::<8>().map(|b| u64::from_le_bytes(*b))
    }

    #[inline(always)]
    pub(crate) fn i64(&mut self) -> Decoded<i64> {
        self.take::<8>().map(|b| i64::from_le_bytes(*b))
    }

    #[inline(always)]
    pub(crate) fn rec(&mut self) -> Decoded<Rec> {
        let rec = Rec::decode(self.take::<REC_LEN>()?);
        if let Some(trace) = &mut self.trace {
            trace.recs.push(rec);
        }
        Ok(rec)
    }

    #[inline(always)]
    pub(crate) fn range(&mut self) -> Decoded<Range> {
        let range = Range {
            start: self.u32()?,
            len: self.u32()?,
        };
        if let Some(trace) = &mut self.trace {
            trace.ranges.push(range);
        }
        Ok(range)
    }

    #[inline(always)]
    pub(crate) fn str(&mut self) -> Decoded<Str> {
        let s = Str {
            off: self.u32()?,
            len: self.u32()?,
        };
        if let Some(trace) = &mut self.trace {
            trace.strs.push(s);
        }
        Ok(s)
    }

    #[inline(always)]
    pub(crate) fn mask(&mut self) -> Decoded<u64> {
        let mask = self.u64()?;
        if let Some(trace) = &mut self.trace {
            trace.masks.push(mask);
        }
        Ok(mask)
    }

    #[inline(always)]
    pub(crate) fn jump(&mut self) -> Decoded<u32> {
        let target = self.u32()?;
        if let Some(trace) = &mut self.trace {
            trace.jumps.push(target);
        }
        Ok(target)
    }

    #[inline(always)]
    pub(crate) fn opt_i32(&mut self) -> Decoded<Option<i32>> {
        let present = self.bool()?;
        let value = self.i32()?;
        Ok(present.then_some(value))
    }

    #[inline(always)]
    pub(crate) fn opt_u32(&mut self) -> Decoded<Option<u32>> {
        let present = self.bool()?;
        let value = self.u32()?;
        Ok(present.then_some(value))
    }

    #[inline(always)]
    pub(crate) fn opt_u64(&mut self) -> Decoded<Option<u64>> {
        let present = self.bool()?;
        let value = self.u64()?;
        Ok(present.then_some(value))
    }

    #[inline(always)]
    pub(crate) fn opt_i64(&mut self) -> Decoded<Option<i64>> {
        let present = self.bool()?;
        let value = self.i64()?;
        Ok(present.then_some(value))
    }
}

#[derive(Default, Debug)]
pub(crate) struct Code {
    pub bytes: Vec<u8>,
}

impl Code {
    #[inline(always)]
    pub(crate) fn pos(&self) -> u32 {
        self.bytes.len() as u32
    }

    pub(crate) fn u8(&mut self, v: u8) {
        self.bytes.push(v);
    }

    pub(crate) fn bool(&mut self, v: bool) {
        self.bytes.push(u8::from(v));
    }

    pub(crate) fn u16(&mut self, v: u16) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn u32(&mut self, v: u32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn i32(&mut self, v: i32) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn u64(&mut self, v: u64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn i64(&mut self, v: i64) {
        self.bytes.extend_from_slice(&v.to_le_bytes());
    }

    pub(crate) fn rec(&mut self, v: Rec) {
        self.bytes.extend_from_slice(&v.encode());
    }

    pub(crate) fn range(&mut self, v: Range) {
        self.u32(v.start);
        self.u32(v.len);
    }

    pub(crate) fn str(&mut self, v: Str) {
        self.u32(v.off);
        self.u32(v.len);
    }

    pub(crate) fn opt_i32(&mut self, v: Option<i32>) {
        self.bool(v.is_some());
        self.i32(v.unwrap_or_default());
    }

    pub(crate) fn opt_u32(&mut self, v: Option<u32>) {
        self.bool(v.is_some());
        self.u32(v.unwrap_or_default());
    }

    pub(crate) fn opt_u64(&mut self, v: Option<u64>) {
        self.bool(v.is_some());
        self.u64(v.unwrap_or_default());
    }

    pub(crate) fn opt_i64(&mut self, v: Option<i64>) {
        self.bool(v.is_some());
        self.i64(v.unwrap_or_default());
    }

    pub(crate) fn patch_u32(&mut self, at: u32, v: u32) {
        let at = at as usize;
        if let Some(slot) = self.bytes.get_mut(at..at + 4) {
            slot.copy_from_slice(&v.to_le_bytes());
        } else {
            debug_assert!(false, "patch outside of code section");
        }
    }
}
