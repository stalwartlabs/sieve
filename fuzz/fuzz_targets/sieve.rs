#![no_main]
use libfuzzer_sys::fuzz_target;

use sieve::{
    runtime::{
        actions::action_notify::{validate_from, validate_uri},
        tests::test_envelope::parse_envelope_address,
    },
    Compiler,
};

static SIEVE_ALPHABET: &[u8] = b"0123abcd;\"\\ {}[](),\n";
static ENVELOPE_ALPHABET: &[u8] = b"0123abcd<>@.";
static URI_ALPHABET: &[u8] = b"abcdefg:@?&;.";
static ADDR_ALPHABET: &[u8] = b"0123abcd<>@.;\"";

fuzz_target!(|data: &[u8]| {
    let data_str = String::from_utf8_lossy(data);

    let compiler = Compiler::new();
    compiler.compile(data).ok();
    compiler.compile(&into_alphabet(data, SIEVE_ALPHABET)).ok();

    parse_envelope_address(&data_str);
    parse_envelope_address(&String::from_utf8(into_alphabet(data, ENVELOPE_ALPHABET)).unwrap());

    validate_from(&data_str);
    validate_from(&String::from_utf8(into_alphabet(data, ADDR_ALPHABET)).unwrap());

    validate_uri(&data_str);
    validate_uri(&String::from_utf8(into_alphabet(data, URI_ALPHABET)).unwrap());
});

fn into_alphabet(data: &[u8], alphabet: &[u8]) -> Vec<u8> {
    data.iter()
        .map(|&byte| alphabet[byte as usize % alphabet.len()])
        .collect()
}
