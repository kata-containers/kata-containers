// Copyright 2024 Sony Group Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{anyhow, Result};
use libcontainer::container::Container;
use liboci_cli::Update;
use oci_spec::runtime::{
    LinuxBlockIoBuilder, LinuxCpuBuilder, LinuxMemoryBuilder, LinuxPidsBuilder, LinuxResources,
};
use protocols::oci::LinuxIntelRdt;
use slog::{info, Logger};
use std::{
    fs::File,
    io::{stdin, Read},
    path::Path,
};

pub fn run(opts: Update, root: &Path, logger: &Logger) -> Result<()> {
    let container = Container::load(root, &opts.container_id)?;

    let mut r: LinuxResources;
    if opts.resources.is_some() {
        let file_path = opts
            .resources
            .ok_or_else(|| anyhow!("Resource file does not exist"))?;
        if file_path.to_str() == Some("-") {
            r = serde_json::from_reader(stdin())?
        } else {
            let mut file = File::open(file_path)?;
            let mut content = String::new();
            file.read_to_string(&mut content)?;
            r = serde_json::from_str(&content)?;
        }
    } else {
        r = LinuxResources::default();

        if opts.pids_limit.is_some() {
            let pids = LinuxPidsBuilder::default()
                .limit(
                    opts.pids_limit
                        .ok_or_else(|| anyhow!("No value provided for --pids-limit"))?,
                )
                .build()?;
            r.set_pids(Some(pids));
        }

        if opts.blkio_weight.is_some() {
            let blkio = LinuxBlockIoBuilder::default()
                .weight(
                    opts.blkio_weight
                        .ok_or_else(|| anyhow!("Invalid value provided for --blkio-weight"))?
                        as u16,
                )
                .build()?;
            r.set_block_io(Some(blkio));
        }

        if opts.cpu_period.is_some()
            || opts.cpu_quota.is_some()
            || opts.cpu_rt_period.is_some()
            || opts.cpu_rt_runtime.is_some()
            || opts.cpu_share.is_some()
            || opts.cpuset_cpus.is_some()
            || opts.cpuset_mems.is_some()
        {
            let mut cpu = LinuxCpuBuilder::default();

            if opts.cpu_period.is_some() {
                cpu = cpu.period(
                    opts.cpu_period
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpu-period"))?,
                );
            }
            if opts.cpu_quota.is_some() {
                cpu = cpu.quota(
                    opts.cpu_quota
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpu-period"))?
                        as i64,
                );
            }
            if opts.cpu_rt_period.is_some() {
                cpu = cpu.realtime_period(
                    opts.cpu_rt_period
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpu-rt-period"))?,
                );
            }
            if opts.cpu_rt_runtime.is_some() {
                cpu = cpu.realtime_runtime(
                    opts.cpu_rt_runtime
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpu-rt-runtime"))?
                        as i64,
                );
            }
            if opts.cpu_share.is_some() {
                cpu = cpu.shares(
                    opts.cpu_share
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpu-share"))?,
                );
            }
            if opts.cpuset_cpus.is_some() {
                cpu = cpu.cpus(
                    opts.cpuset_cpus
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpuset-cpus"))?,
                );
            }
            if opts.cpuset_mems.is_some() {
                cpu = cpu.mems(
                    opts.cpuset_mems
                        .ok_or_else(|| anyhow!("Invalid value provided for --cpuset-mems"))?,
                );
            }
            r.set_cpu(Some(cpu.build()?));
        }

        if opts.memory.is_some() || opts.memory_reservation.is_some() || opts.memory_swap.is_some()
        {
            let mut memory = LinuxMemoryBuilder::default();
            if opts.memory.is_some() {
                memory = memory.limit(
                    opts.memory
                        .ok_or_else(|| anyhow!("Invalid value provided for --memory-reservation"))?
                        as i64,
                );
            }
            if opts.memory_reservation.is_some() {
                memory = memory.reservation(
                    opts.memory_reservation
                        .ok_or_else(|| anyhow!("Invalid value provided for --memory-reservation"))?
                        as i64,
                );
            }
            if opts.memory_swap.is_some() {
                memory = memory.swap(
                    opts.memory_swap
                        .ok_or_else(|| anyhow!("Invalid value provided for --memory_swap"))?
                        as i64,
                );
            }
            r.set_memory(Some(memory.build()?));
        }
    }

    if r != LinuxResources::default() {
        container.update(r, logger)?;
    }

    if opts.l3_cache_schema.is_some() {
        let mut linux_intel_rdt = LinuxIntelRdt::default();
        linux_intel_rdt.set_L3CacheSchema(
            opts.l3_cache_schema
                .ok_or_else(|| anyhow!("No value provided for --l3-cache-schema"))?,
        );
    }

    info!(&logger, "update command finished successfully");
    Ok(())
}
