/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use super::{
    Corrupt, Decoded,
    cursor::Cursor,
    ops::{self, skip_op},
    rec::{Range, Rec, tag},
};
use crate::{
    Sieve,
    compiler::grammar::{actions::action_set::MODIFIER_REPLACE, expr::parser::ID_EXTERNAL},
};

const MAX_NESTING: u8 = 8;

pub(crate) fn verify(sieve: &Sieve<'_>) -> Decoded<()> {
    let code = sieve.code();
    let mut cursor = Cursor::tracing(code);
    let mut boundaries = Boundaries::new(code.len() + 1);
    let mut jumps = Vec::new();

    while !cursor.at_end() {
        boundaries.set(cursor.pos);
        let op = cursor.u8()?;
        if op == ops::Clear::OP {
            verify_clear(sieve, &mut Cursor::new(code, cursor.pos))?;
        }
        skip_op(op, &mut cursor)?;
        let trace = cursor.trace_mut();
        for mask in trace.masks.drain(..) {
            verify_mask(sieve, mask)?;
        }
        for range in trace.ranges.drain(..) {
            verify_range(sieve, range, 0)?;
        }
        for s in trace.strs.drain(..) {
            sieve.str(s)?;
        }
        for rec in trace.recs.drain(..) {
            let mut iter = std::iter::empty();
            verify_rec(sieve, rec, &mut iter, 0)?;
        }
        jumps.append(&mut trace.jumps);
    }
    boundaries.set(code.len());

    if jumps.iter().all(|target| boundaries.get(*target as usize)) {
        Ok(())
    } else {
        Err(Corrupt)
    }
}

fn verify_clear(sieve: &Sieve<'_>, cursor: &mut Cursor<'_>) -> Decoded<()> {
    let clear = ops::Clear::decode(cursor)?;
    let end = clear
        .local_vars_idx
        .checked_add(clear.local_vars_num)
        .ok_or(Corrupt)?;
    if end as usize > sieve.num_vars() {
        return Err(Corrupt);
    }
    verify_mask(sieve, clear.match_vars)
}

fn verify_mask(sieve: &Sieve<'_>, mask: u64) -> Decoded<()> {
    let highest = (u64::BITS - mask.leading_zeros()) as usize;
    if highest <= sieve.num_match_vars() {
        Ok(())
    } else {
        Err(Corrupt)
    }
}

struct Boundaries {
    bits: Vec<u64>,
}

impl Boundaries {
    fn new(len: usize) -> Self {
        Boundaries {
            bits: vec![0; len.div_ceil(64)],
        }
    }

    #[inline(always)]
    fn set(&mut self, at: usize) {
        if let Some(word) = self.bits.get_mut(at / 64) {
            *word |= 1 << (at % 64);
        }
    }

    #[inline(always)]
    fn get(&self, at: usize) -> bool {
        self.bits
            .get(at / 64)
            .is_some_and(|word| word & (1 << (at % 64)) != 0)
    }
}

fn verify_range(sieve: &Sieve<'_>, range: Range, depth: u8) -> Decoded<()> {
    if depth > MAX_NESTING {
        return Err(Corrupt);
    }
    let mut iter = sieve.recs(range)?;
    while let Some(rec) = iter.next() {
        if rec.tag == tag::JMP_IF {
            let remaining = range.start + range.len - iter.index;
            if rec.d > remaining {
                return Err(Corrupt);
            }
        }
        verify_rec(sieve, rec, &mut iter, depth)?;
    }
    Ok(())
}

fn verify_rec(
    sieve: &Sieve<'_>,
    rec: Rec,
    following: &mut impl Iterator<Item = Rec>,
    depth: u8,
) -> Decoded<()> {
    match rec.tag {
        tag::CALL => {
            if rec.d <= ID_EXTERNAL {
                Ok(())
            } else {
                Err(Corrupt)
            }
        }
        tag::VAR_LOCAL => {
            if (rec.c as usize) < sieve.num_vars() {
                Ok(())
            } else {
                Err(Corrupt)
            }
        }
        tag::VAR_MATCH => {
            if (rec.b as usize) < sieve.num_match_vars() {
                Ok(())
            } else {
                Err(Corrupt)
            }
        }
        tag::NONE
        | tag::INT
        | tag::FLOAT
        | tag::VAR_ENVELOPE
        | tag::VAR_PART
        | tag::BIN_OP
        | tag::UN_OP
        | tag::JMP_IF
        | tag::ARRAY_ACCESS
        | tag::ARRAY_BUILD
        | tag::ENVELOPE
        | tag::NOTIFY_ITEM
        | tag::VARIABLE_NONE => Ok(()),
        tag::TEXT | tag::VAR_GLOBAL | tag::VAR_ENV => sieve.str(rec.str()).map(|_| ()),
        tag::REGEX => {
            sieve.str(rec.str())?;
            if (rec.c as u32) < sieve.num_regexes() {
                Ok(())
            } else {
                Err(Corrupt)
            }
        }
        tag::GLOB => {
            sieve.str(rec.str())?;
            sieve.glob(rec.c).map(|_| ())
        }
        tag::HEADER => sieve.header_name(rec.c).map(|_| ()),
        tag::LIST => verify_range(sieve, rec.range(), depth + 1),
        tag::REF => verify_range(
            sieve,
            Range {
                start: rec.d,
                len: (rec.e as u32).saturating_add(1),
            },
            depth + 1,
        ),
        tag::VAR_HEADER => {
            let cont = following.next().ok_or(Corrupt)?;
            if cont.tag != tag::CONT {
                return Err(Corrupt);
            }
            let names = Range {
                start: rec.d,
                len: cont.c as u32,
            };
            for name in sieve.recs(names)? {
                if name.tag != tag::HEADER {
                    return Err(Corrupt);
                }
                sieve.header_name(name.c)?;
            }
            if cont.e != 0 {
                sieve.str(cont.str())?;
            }
            Ok(())
        }
        tag::CONT => Err(Corrupt),
        tag::MODIFIER => {
            if rec.b == MODIFIER_REPLACE {
                for _ in 0..2 {
                    let value = following.next().ok_or(Corrupt)?;
                    verify_rec(sieve, value, &mut std::iter::empty(), depth + 1)?;
                }
            }
            Ok(())
        }
        tag::CAPABILITY => {
            if rec.e != 0 {
                sieve.str(rec.str())?;
            }
            Ok(())
        }
        _ => Err(Corrupt),
    }
}
