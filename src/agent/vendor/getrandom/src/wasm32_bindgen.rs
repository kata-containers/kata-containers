// Copyright 2018 Developers of the Rand project.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// https://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or https://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

//! Implementation for WASM via wasm-bindgen
extern crate std;

use core::cell::RefCell;
use core::mem;
use std::thread_local;

use wasm_bindgen::prelude::*;

use crate::error::{BINDGEN_CRYPTO_UNDEF, BINDGEN_GRV_UNDEF};
use crate::Error;

#[derive(Clone, Debug)]
enum RngSource {
    Node(NodeCrypto),
    Browser(BrowserCrypto),
}

// JsValues are always per-thread, so we initialize RngSource for each thread.
//   See: https://github.com/rustwasm/wasm-bindgen/pull/955
thread_local!(
    static RNG_SOURCE: RefCell<Option<RngSource>> = RefCell::new(None);
);

pub fn getrandom_inner(dest: &mut [u8]) -> Result<(), Error> {
    assert_eq!(mem::size_of::<usize>(), 4);

    RNG_SOURCE.with(|f| {
        let mut source = f.borrow_mut();
        if source.is_none() {
            *source = Some(getrandom_init()?);
        }

        match source.as_ref().unwrap() {
            RngSource::Node(n) => n.random_fill_sync(dest),
            RngSource::Browser(n) => {
                // see https://developer.mozilla.org/en-US/docs/Web/API/Crypto/getRandomValues
                //
                // where it says:
                //
                // > A QuotaExceededError DOMException is thrown if the
                // > requested length is greater than 65536 bytes.
                for chunk in dest.chunks_mut(65536) {
                    n.get_random_values(chunk)
                }
            }
        };
        Ok(())
    })
}

fn getrandom_init() -> Result<RngSource, Error> {
    if let Ok(self_) = Global::get_self() {
        // If `self` is defined then we're in a browser somehow (main window
        // or web worker). We get `self.crypto` (called `msCrypto` on IE), so we
        // can call `crypto.getRandomValues`. If `crypto` isn't defined, we
        // assume we're in an older web browser and the OS RNG isn't available.

        let crypto: BrowserCrypto = match (self_.crypto(), self_.ms_crypto()) {
            (crypto, _) if !crypto.is_undefined() => crypto.into(),
            (_, crypto) if !crypto.is_undefined() => crypto.into(),
            _ => return Err(BINDGEN_CRYPTO_UNDEF),
        };

        // Test if `crypto.getRandomValues` is undefined as well
        if crypto.get_random_values_fn().is_undefined() {
            return Err(BINDGEN_GRV_UNDEF);
        }

        return Ok(RngSource::Browser(crypto));
    }

    return Ok(RngSource::Node(MODULE.require("crypto")));
}

#[wasm_bindgen]
extern "C" {
    type Global;
    #[wasm_bindgen(getter, catch, static_method_of = Global, js_class = self, js_name = self)]
    fn get_self() -> Result<Self_, JsValue>;

    type Self_;
    #[wasm_bindgen(method, getter, js_name = "msCrypto", structural)]
    fn ms_crypto(me: &Self_) -> JsValue;
    #[wasm_bindgen(method, getter, structural)]
    fn crypto(me: &Self_) -> JsValue;

    #[derive(Clone, Debug)]
    type BrowserCrypto;

    // TODO: these `structural` annotations here ideally wouldn't be here to
    // avoid a JS shim, but for now with feature detection they're
    // unavoidable.
    #[wasm_bindgen(method, js_name = getRandomValues, structural, getter)]
    fn get_random_values_fn(me: &BrowserCrypto) -> JsValue;
    #[wasm_bindgen(method, js_name = getRandomValues, structural)]
    fn get_random_values(me: &BrowserCrypto, buf: &mut [u8]);

    #[derive(Clone, Debug)]
    type NodeCrypto;

    #[wasm_bindgen(method, js_name = randomFillSync, structural)]
    fn random_fill_sync(me: &NodeCrypto, buf: &mut [u8]);

    type NodeModule;

    #[wasm_bindgen(js_name = module)]
    static MODULE: NodeModule;

    #[wasm_bindgen(method)]
    fn require(this: &NodeModule, s: &str) -> NodeCrypto;
}
