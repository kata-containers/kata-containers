// Copyright (c) 2022-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::Result;
use std::process::exit;
use wasmtime::{Engine, Linker, Module, Store};
use wasmtime_wasi::WasiCtxBuilder;

const WASM_MODULE: &str = "KATA_WASM";

pub fn wasm_task(args: &[String]) -> ! {
    let _ = run_wasmtime(args).map_err(|_| exit(-1));

    exit(0);
}

/// https://docs.wasmtime.dev/examples-rust-wasi.html
fn run_wasmtime(args: &[String]) -> Result<()> {
    let engine = Engine::default();
    let mut linker = Linker::new(&engine);
    wasmtime_wasi::add_to_linker(&mut linker, |s| s)?;

    let wasi = WasiCtxBuilder::new().inherit_stdio().args(args)?.build();
    let mut store = Store::new(&engine, wasi);

    let module = Module::from_file(&engine, args[0].clone())?;
    linker.module(&mut store, WASM_MODULE, &module)?;
    linker
        .get_default(&mut store, WASM_MODULE)?
        .typed::<(), ()>(&store)?
        .call(&mut store, ())?;

    Ok(())
}
