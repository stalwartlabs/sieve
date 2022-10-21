use crate::Sieve;

const SIEVE_MARKER: u8 = 0xff;
const SIEVE_FORMAT_VERSION: u8 = 1;

pub enum SerializeError {
    Other,
}

impl Sieve {
    pub fn deserialize(bytes: &[u8]) -> Result<Self, Box<bincode::ErrorKind>> {
        if bytes.len() > 2 && bytes[0] == SIEVE_MARKER && bytes[1] == SIEVE_FORMAT_VERSION {
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
        buf.push(SIEVE_FORMAT_VERSION);
        bincode::serialize_into(&mut buf, self)?;
        Ok(buf)
    }
}
