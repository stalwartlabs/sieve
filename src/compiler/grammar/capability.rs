use phf::phf_map;
use serde::{Deserialize, Serialize};

use super::comparator::Comparator;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Capability {
    Envelope,
    EnvelopeDsn,
    EnvelopeDeliverBy,
    FileInto,
    EncodedCharacter,
    Comparator(Comparator),
    Other(String),
    Body,
    Convert,
    Copy,
    Relational,
    Date,
    Index,
    Duplicate,
    Variables,
    EditHeader,
    ForEveryPart,
    Mime,
    Replace,
    Enclose,
    ExtractText,
    Enotify,
    RedirectDsn,
    RedirectDeliverBy,
}

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
    "envelope-dsn" => Capability::EnvelopeDsn,
    "envelope-deliverby" => Capability::EnvelopeDeliverBy,
    "fileinto" => Capability::FileInto,
    "encoded-character" => Capability::EncodedCharacter,
    "comparator-elbonia" => Capability::Comparator(Comparator::Elbonia),
    "comparator-i;octet" => Capability::Comparator(Comparator::Octet),
    "comparator-i;ascii-casemap" => Capability::Comparator(Comparator::AsciiCaseMap),
    "comparator-i;ascii-numeric" => Capability::Comparator(Comparator::AsciiNumeric),
    "body" => Capability::Body,
    "convert" => Capability::Convert,
    "copy" => Capability::Copy,
    "relational" => Capability::Relational,
    "date" => Capability::Date,
    "index" => Capability::Index,
    "duplicate" => Capability::Duplicate,
    "variables" => Capability::Variables,
    "editheader" => Capability::EditHeader,
    "foreverypart" => Capability::ForEveryPart,
    "mime" => Capability::Mime,
    "replace" => Capability::Replace,
    "enclose" => Capability::Enclose,
    "extracttext" => Capability::ExtractText,
    "enotify" => Capability::Enotify,
    "redirect-dsn" => Capability::RedirectDsn,
    "redirect-deliverby" => Capability::RedirectDeliverBy,
};
