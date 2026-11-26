// Copyright (c) 2019-2023 Alibaba Cloud
// Copyright (c) 2019-2023 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;

use anyhow::{Ok, Result};

use kata_types::cpu::LinuxContainerCpuResources;
use oci_spec::runtime::LinuxResources;

use crate::cpu_mem::{cpu::CpuResource, mem::MemResource};

pub mod cpu;
pub mod initial_size;
pub mod mem;
pub mod swap;

// Commit in-memory adoption only after the fallible host update succeeds.
// Holding these guards also leaves no cancellation point between map writes.
pub async fn adopt_cpu_mem_accounting(
    cpu: &CpuResource,
    mem: &MemResource,
    cid: &str,
    resources: Option<&LinuxResources>,
    vcpus: u32,
    host_update: impl std::future::Future<Output = Result<()>>,
) -> Result<()> {
    let container_cpu = resources
        .and_then(|r| r.cpu().as_ref())
        .map(LinuxContainerCpuResources::try_from)
        .transpose()?;
    let mut cpus = cpu.container_cpu_resources.write().await;
    let mut mems = mem.container_mem_resources.write().await;
    let mut current_vcpu = cpu.current_vcpu.write().await;

    host_update.await?;

    if let Some(value) = container_cpu {
        cpus.insert(cid.to_owned(), value);
    }
    if let Some(value) = resources {
        mems.insert(cid.to_owned(), value.clone());
    }
    *current_vcpu = vcpus as f32;

    Ok(())
}
