use phf::phf_map;

use crate::{Capability, Comparator};

impl Capability {
    pub fn parse(bytes: Vec<u8>) -> Capability {
        if let Some(capability) = CAPABILITIES.get(std::str::from_utf8(&bytes).unwrap_or("")) {
            capability.clone()
        } else {
            let capability = String::from_utf8(bytes)
                .unwrap_or_else(|err| String::from_utf8_lossy(err.as_bytes()).into_owned());
            if let Some(comparator) = capability.strip_prefix("comparator-") {
                Capability::Comparator(Comparator::Other(comparator.to_string()))
            } else {
                Capability::Other(capability)
            }
        }
    }
}

static CAPABILITIES: phf::Map<&'static str, Capability> = phf_map! {
    "envelope" => Capability::Envelope,
    "fileinto" => Capability::FileInto,
    "encoded-character" => Capability::EncodedCharacter,
    "comparator-elbonia" => Capability::Comparator(Comparator::Elbonia),
    "comparator-i;octet" => Capability::Comparator(Comparator::Octet),
    "comparator-i;ascii-casemap" => Capability::Comparator(Comparator::AsciiCaseMap),
    "comparator-i;ascii-numeric" => Capability::Comparator(Comparator::AsciiNumeric),
};
