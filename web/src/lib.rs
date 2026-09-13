/*
 * SPDX-FileCopyrightText: 2020 Stalwart Labs LLC <hello@stalw.art>
 *
 * SPDX-License-Identifier: AGPL-3.0-only
 */

pub mod functions;
pub mod handler;
pub mod output;
pub mod run;
pub mod settings;
#[cfg(test)]
mod tests;

use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

use crate::{run::Request, settings::Settings};

#[wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
}

#[wasm_bindgen]
pub fn version() -> String {
    env!("SIEVE_VERSION").to_string()
}

#[wasm_bindgen]
pub fn capabilities() -> Result<JsValue, JsError> {
    let names: Vec<String> = settings::all_capabilities()
        .map(|capability| capability.to_string())
        .collect();
    to_js(&names)
}

#[wasm_bindgen]
pub fn defaults() -> Result<JsValue, JsError> {
    to_js(&Settings::default())
}

#[wasm_bindgen]
pub fn compile(request: JsValue) -> Result<JsValue, JsError> {
    let request: Request = serde_wasm_bindgen::from_value(request)?;
    to_js(&request.compile())
}

#[wasm_bindgen]
pub fn run(request: JsValue) -> Result<JsValue, JsError> {
    let request: Request = serde_wasm_bindgen::from_value(request)?;
    to_js(&request.run())
}

fn to_js(value: &impl Serialize) -> Result<JsValue, JsError> {
    value
        .serialize(&Serializer::json_compatible())
        .map_err(Into::into)
}
