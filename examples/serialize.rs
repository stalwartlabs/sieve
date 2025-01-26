/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs Ltd <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only OR LicenseRef-SEL
 */

use sieve::{Compiler, Sieve};

fn main() {
    let script = br#"if header :matches \"List-ID\" \"*<*@*\" {
        fileinto \"INBOX.lists.${2}\"; stop;
    }"#;

    // Compile
    let compiled_script = Compiler::new().compile(script).unwrap();

    // Serialize
    let serialized_script = compiled_script.serialize().unwrap();

    // Deserialize
    let deserialized_script = Sieve::deserialize(&serialized_script).unwrap();

    assert_eq!(compiled_script, deserialized_script);
}
