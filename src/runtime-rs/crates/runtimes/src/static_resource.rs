// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::convert::TryFrom;

use anyhow::{Context, Result};

use kata_types::{
    annotations::Annotation, config::TomlConfig, container::ContainerType,
    cpu::LinuxContainerCpuResources, k8s::container_type,
};

// static resource that StaticResourceManager needs, this is the spec for the
// sandbox/container's workload
#[derive(Clone, Copy, Debug)]
struct StaticResource {
    vcpu: u32,
    mem_mb: u32,
}

// generate static resource(vcpu and memory in MiB) from spec's information
// used for static resource management
impl TryFrom<&oci::Spec> for StaticResource {
    type Error = anyhow::Error;
    fn try_from(spec: &oci::Spec) -> Result<Self> {
        let mut vcpu: u32 = 0;
        let mut mem_mb: u32 = 0;
        match container_type(spec) {
            // podsandbox, from annotation
            ContainerType::PodSandbox => {
                let annotation = Annotation::new(spec.annotations.clone());
                let (period, quota, memory) =
                    get_sizing_info(annotation).context("failed to get sizing info")?;
                let cpu = oci::LinuxCpu {
                    period: Some(period),
                    quota: Some(quota),
                    ..Default::default()
                };
                // although it may not be actually a linux container, we are only using the calculation inside
                // LinuxContainerCpuResources::try_from to generate our vcpu number
                if let Ok(cpu_resource) = LinuxContainerCpuResources::try_from(&cpu) {
                    vcpu = get_nr_vcpu(&cpu_resource);
                }
                mem_mb = convert_memory_to_mb(memory);
            }
            // single container, from container spec
            _ => {
                if let Some(linux) = &spec.linux {
                    if let Some(resource) = &linux.resources {
                        if let Some(cpu) = &resource.cpu {
                            if let Ok(cpu_resource) = LinuxContainerCpuResources::try_from(cpu) {
                                vcpu = get_nr_vcpu(&cpu_resource);
                            }
                        }
                        if let Some(mem) = &resource.memory {
                            let memory = mem.limit.unwrap_or(0);
                            mem_mb = convert_memory_to_mb(memory);
                        }
                    }
                }
            }
        }
        info!(
            sl!(),
            "static resource mgmt result: vcpu={}, mem_mb={}", vcpu, mem_mb
        );
        Ok(Self { vcpu, mem_mb })
    }
}

// StaticResourceManager is responsible for static resource management
//
// static resource management sizing information is optionally provided, either by
// upper layer runtime (containerd / crio) or by the container spec itself (when it
// is a standalone single container such as the one started with *docker run*)
//
// the sizing information uses three values, cpu quota, cpu period and memory limit,
// and with above values it calculates the # vcpus and memory for the workload and
// add them to default value of the config
#[derive(Clone, Copy, Debug)]
pub struct StaticResourceManager {
    resource: StaticResource,
}

impl StaticResourceManager {
    pub fn new(spec: &oci::Spec) -> Result<Self> {
        Ok(Self {
            resource: StaticResource::try_from(spec)
                .context("failed to construct static resource")?,
        })
    }

    pub fn setup_config(&self, config: &mut TomlConfig) -> Result<()> {
        // update this data to the hypervisor config for later use by hypervisor
        let hypervisor_name = &config.runtime.hypervisor_name;
        let mut hv = config
            .hypervisor
            .get_mut(hypervisor_name)
            .context("failed to get hypervisor config")?;
        hv.cpu_info.default_vcpus += self.resource.vcpu as i32;
        hv.memory_info.default_memory += self.resource.mem_mb;
        Ok(())
    }
}

fn get_nr_vcpu(resource: &LinuxContainerCpuResources) -> u32 {
    if let Some(v) = resource.get_vcpus() {
        v as u32
    } else {
        0
    }
}

fn convert_memory_to_mb(memory_in_byte: i64) -> u32 {
    if memory_in_byte < 0 {
        0
    } else {
        (memory_in_byte / 1024 / 1024) as u32
    }
}

// from the upper layer runtime's annotation (e.g. crio, k8s), get the *cpu quota,
// cpu period and memory limit* for a sandbox/container
fn get_sizing_info(annotation: Annotation) -> Result<(u64, i64, i64)> {
    // since we are *adding* our result to the config, a value of 0 will cause no change
    // and if the annotation is not assigned (but static resource management is), we will
    // log a *warning* to fill that with zero value
    let period = annotation.get_sandbox_cpu_period();
    let quota = annotation.get_sandbox_cpu_quota();
    let memory = annotation.get_sandbox_mem();
    Ok((period, quota, memory))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_static_resource_mgmt() {
        // data below should result in 2200 mCPU(round up to 3 vcpus) and 512 MiB of memory
        let period: u64 = 100000;
        let memory: i64 = 1048576 * 512; // 512 MiB
        let quota: i64 = 220000;

        let cpu = oci::LinuxCpu {
            period: Some(period),
            quota: Some(quota),
            ..Default::default()
        };
        if let Ok(cpu_resource) = LinuxContainerCpuResources::try_from(&cpu) {
            if let Some(v) = cpu_resource.get_vcpus() {
                assert_eq!(v, 3);
            }
        }

        let mem_mb = if memory < 0 {
            0
        } else {
            (memory / 1024 / 1024) as u32
        };

        assert_eq!(mem_mb, 512);
    }
}
