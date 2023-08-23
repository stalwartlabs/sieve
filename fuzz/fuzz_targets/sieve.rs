/*
 * Copyright (c) 2020-2023, Stalwart Labs Ltd.
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

#![no_main]
use libfuzzer_sys::fuzz_target;

use sieve::{
    runtime::{
        actions::action_notify::{validate_from, validate_uri},
        tests::test_envelope::parse_envelope_address,
    },
    Compiler,
};

use sieve::compiler::{
    grammar::expr::{parser::ExpressionParser, tokenizer::Tokenizer},
    VariableType,
};

static SIEVE_ALPHABET: &[u8] = b"0123abcd;\"\\ {}[](),\n";
static ENVELOPE_ALPHABET: &[u8] = b"0123abcd<>@.";
static URI_ALPHABET: &[u8] = b"abcdefg:@?&;.";
static ADDR_ALPHABET: &[u8] = b"0123abcd<>@.;\"";
static EXPR_ALPHABET: &[u8] = b"01235+-*/!&|<>=.()";

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

    ExpressionParser::from_tokenizer(Tokenizer::new(&data_str, |_, _| Ok(VariableType::Local(0))))
        .parse()
        .ok();

    ExpressionParser::from_tokenizer(Tokenizer::new(
        &String::from_utf8(into_alphabet(data, EXPR_ALPHABET)).unwrap(),
        |_, _| Ok(VariableType::Local(0)),
    ))
    .parse()
    .ok();
});

fn into_alphabet(data: &[u8], alphabet: &[u8]) -> Vec<u8> {
    data.iter()
        .map(|&byte| alphabet[byte as usize % alphabet.len()])
        .collect()
}
