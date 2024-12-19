// Copyright (C) 2023 Ant group. All rights reserved.
//
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Result};
use share::option::{CompactSetOption, MemcgSetOption};
use slog::{Drain, Level, Logger};
use slog_async;
use slog_scope::set_global_logger;
use slog_scope::{error, info};
use slog_term;
use std::fs::OpenOptions;
use std::io::BufWriter;
use structopt::StructOpt;

mod protocols;
mod share;

#[derive(StructOpt, Debug)]
#[structopt(name = "mem-agent", about = "Memory agent")]
struct Opt {
    #[structopt(long, default_value = "unix:///var/run/mem-agent.sock")]
    addr: String,
    #[structopt(long)]
    log_file: Option<String>,
    #[structopt(long, default_value = "trace", parse(try_from_str = parse_slog_level))]
    log_level: Level,
    #[structopt(flatten)]
    memcg: MemcgSetOption,
    #[structopt(flatten)]
    compact: CompactSetOption,
}

fn parse_slog_level(src: &str) -> Result<Level, String> {
    match src.to_lowercase().as_str() {
        "trace" => Ok(Level::Trace),
        "debug" => Ok(Level::Debug),
        "info" => Ok(Level::Info),
        "warning" => Ok(Level::Warning),
        "warn" => Ok(Level::Warning),
        "error" => Ok(Level::Error),
        _ => Err(format!("Invalid log level: {}", src)),
    }
}

fn setup_logging(opt: &Opt) -> Result<slog_scope::GlobalLoggerGuard> {
    let drain = if let Some(f) = &opt.log_file {
        let log_file = OpenOptions::new()
            .create(true)
            .write(true)
            .append(true)
            .open(f)
            .map_err(|e| anyhow!("Open log file {} fail: {}", f, e))?;
        let buffered = BufWriter::new(log_file);
        let decorator = slog_term::PlainDecorator::new(buffered);
        let drain = slog_term::CompactFormat::new(decorator)
            .build()
            .filter_level(opt.log_level)
            .fuse();
        slog_async::Async::new(drain).build().fuse()
    } else {
        let decorator = slog_term::TermDecorator::new().stderr().build();
        let drain = slog_term::CompactFormat::new(decorator)
            .build()
            .filter_level(opt.log_level)
            .fuse();
        slog_async::Async::new(drain).build().fuse()
    };

    let logger = Logger::root(drain, slog::o!());
    Ok(set_global_logger(logger.clone()))
}

fn main() -> Result<()> {
    // Check opt
    let opt = Opt::from_args();

    let _ = setup_logging(&opt).map_err(|e| anyhow!("setup_logging fail: {}", e))?;

    let memcg_config = opt.memcg.to_mem_agent_memcg_config();
    let compact_config = opt.compact.to_mem_agent_compact_config();

    let (ma, _rt) = mem_agent::agent::MemAgent::new(memcg_config, compact_config)
        .map_err(|e| anyhow!("MemAgent::new fail: {}", e))?;

    info!("MemAgent started");

    share::rpc::rpc_loop(ma, opt.addr).map_err(|e| {
        let estr = format!("rpc::rpc_loop fail: {}", e);
        error!("{}", estr);
        anyhow!("{}", estr)
    })?;

    Ok(())
}
