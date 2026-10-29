// Copyright (c) 2026 Kata Containers community
// SPDX-License-Identifier: Apache-2.0

mod installer_config;

use installer_config::render_installer_config;
use std::env;
use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::PathBuf;

fn main() {
    if let Err(error) = run() {
        eprintln!("render-cni-config: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args: Vec<OsString> = env::args_os().skip(1).collect();
    if args.len() != 4 {
        return Err(
            "usage: render-cni-config TEMPLATE NODE_NAME RUNTIME_CLASS DAN_CONF_DIR".into(),
        );
    }
    let template = fs::read(PathBuf::from(&args[0]))
        .map_err(|error| format!("cannot read Kata DAN template: {error}"))?;
    let node_name = args[1].to_str().ok_or("node name must be UTF-8")?;
    let runtime_class = args[2].to_str().ok_or("RuntimeClass must be UTF-8")?;
    let dan_conf_dir = args[3].to_str().ok_or("DAN directory must be UTF-8")?;
    let rendered = render_installer_config(&template, node_name, runtime_class, dan_conf_dir)?;
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serde_json::to_writer_pretty(&mut writer, &rendered)
        .map_err(|error| format!("cannot write rendered conflist: {error}"))?;
    writer
        .write_all(b"\n")
        .map_err(|error| format!("cannot finish rendered conflist: {error}"))
}
