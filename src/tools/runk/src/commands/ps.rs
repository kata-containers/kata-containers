// Copyright 2021-2022 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::anyhow;
use anyhow::Result;
use libcontainer::container::Container;
use liboci_cli::Ps;
use slog::{info, Logger};
use std::path::Path;
use std::process::Command;
use std::str;

pub fn run(opts: Ps, root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(root, opts.container_id.as_str())?;
    let pids = container
        .processes()?
        .iter()
        .map(|pid| pid.as_raw())
        .collect::<Vec<_>>();

    match opts.format.as_str() {
        "json" => println!("{}", serde_json::to_string(&pids)?),
        "table" => {
            let ps_options = if opts.ps_options.is_empty() {
                vec!["-ef".to_string()]
            } else {
                opts.ps_options
            };
            let output = Command::new("ps").args(ps_options).output()?;
            if !output.status.success() {
                return Err(anyhow!("{}", std::str::from_utf8(&output.stderr)?));
            }
            let lines = str::from_utf8(&output.stdout)?.lines().collect::<Vec<_>>();
            if lines.is_empty() {
                return Err(anyhow!("no processes found"));
            }
            let pid_index = lines[0]
                .split_whitespace()
                .position(|field| field == "PID")
                .ok_or_else(|| anyhow!("could't find PID field in ps output"))?;
            println!("{}", lines[0]);
            for &line in &lines[1..] {
                if line.is_empty() {
                    continue;
                }
                let fields = line.split_whitespace().collect::<Vec<_>>();
                if pid_index >= fields.len() {
                    continue;
                }
                let pid: i32 = fields[pid_index].parse()?;
                if pids.contains(&pid) {
                    println!("{}", line);
                }
            }
        }
        _ => return Err(anyhow!("unknown format: {}", opts.format)),
    }

    info!(&logger, "ps command finished successfully");
    Ok(())
}
