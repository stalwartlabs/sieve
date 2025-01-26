/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use crate::{Compiler, Sieve};

const SIEVE_MARKER: u8 = 0xff;

pub enum SerializeError {
    Other,
}

impl Sieve {
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Box<bincode::ErrorKind>> {
        if bytes.len() > 2 && bytes[0] == SIEVE_MARKER && bytes[1] == Compiler::VERSION as u8 {
            bincode::deserialize(&bytes[2..])
        } else {
            Err(Box::new(bincode::ErrorKind::Custom(
                "Incompatible version".to_string(),
            )))
        }
    }

    pub fn serialize(&self) -> Result<Vec<u8>, Box<bincode::ErrorKind>> {
        let mut buf = Vec::with_capacity(bincode::serialized_size(self)? as usize + 2);
        buf.push(SIEVE_MARKER);
        buf.push(Compiler::VERSION as u8);
        bincode::serialize_into(&mut buf, self)?;
        Ok(buf)
    }
}
