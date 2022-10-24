/*
 * Copyright (c) 2020-2022, Stalwart Labs Ltd.
 *
 * This file is part of the Stalwart Sieve Interpreter.
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as
 * published by the Free Software Foundation, either version 3 of
 * the License, or (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU Affero General Public License for more details.
 * in the LICENSE file at the top-level directory of this distribution.
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 *
 * You can be released from the requirements of the AGPLv3 license by
 * purchasing a commercial license. Please contact licensing@stalw.art
 * for more details.
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
