// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, convert::TryFrom};

use anyhow::{Context, Result};
use kata_types::{
    annotations::Annotation, config::TomlConfig, container::ContainerType,
    cpu::LinuxContainerCpuResources, k8s::container_type,
};
use oci_spec::runtime as oci;

// initial resource that InitialSizeManager needs, this is the spec for the
// sandbox/container's workload
#[derive(Clone, Copy, Debug)]
struct InitialSize {
    vcpu: u32,
    mem_mb: u32,
    orig_toml_default_mem: u32,
}

// generate initial resource(vcpu and memory in MiB) from annotations
impl TryFrom<&HashMap<String, String>> for InitialSize {
    type Error = anyhow::Error;
    fn try_from(an: &HashMap<String, String>) -> Result<Self> {
        let mut vcpu: u32 = 0;

        let annotation = Annotation::new(an.clone());
        let (period, quota, memory) =
            get_sizing_info(annotation).context("failed to get sizing info")?;
        let mut cpu = oci::LinuxCpu::default();
        cpu.set_period(Some(period));
        cpu.set_quota(Some(quota));

        // although it may not be actually a linux container, we are only using the calculation inside
        // LinuxContainerCpuResources::try_from to generate our vcpu number
        if let Ok(cpu_resource) = LinuxContainerCpuResources::try_from(&cpu) {
            vcpu = get_nr_vcpu(&cpu_resource);
        }
        let mem_mb = convert_memory_to_mb(memory);

        Ok(Self {
            vcpu,
            mem_mb,
            orig_toml_default_mem: 0,
        })
    }
}

// generate initial resource(vcpu and memory in MiB) from spec's information
impl TryFrom<&oci::Spec> for InitialSize {
    type Error = anyhow::Error;
    fn try_from(spec: &oci::Spec) -> Result<Self> {
        let mut vcpu: u32 = 0;
        let mut mem_mb: u32 = 0;
        match container_type(spec) {
            // podsandbox, from annotation
            ContainerType::PodSandbox => {
                let spec_annos = spec.annotations().clone().unwrap_or_default();
                return InitialSize::try_from(&spec_annos);
            }
            // single container, from container spec
            _ => {
                if let Some(resource) = spec
                    .linux()
                    .as_ref()
                    .and_then(|linux| linux.resources().as_ref())
                {
                    // cpu resource
                    if let Some(Ok(cpu_resource)) = resource
                        .cpu()
                        .as_ref()
                        .map(LinuxContainerCpuResources::try_from)
                    {
                        vcpu = get_nr_vcpu(&cpu_resource);
                    }

                    // memory resource
                    mem_mb = resource
                        .memory()
                        .as_ref()
                        .and_then(|mem| mem.limit())
                        .map(convert_memory_to_mb)
                        .unwrap_or(0);
                }
            }
        }
        info!(
            sl!(),
            "(from PodSandbox's annotation / SingleContainer's spec) initial size: vcpu={}, mem_mb={}", vcpu, mem_mb
        );
        Ok(Self {
            vcpu,
            mem_mb,
            orig_toml_default_mem: 0,
        })
    }
}

// InitialSizeManager is responsible for initial vcpu/mem management
//
// inital vcpu/mem management sizing information is optionally provided, either by
// upper layer runtime (containerd / crio) or by the container spec itself (when it
// is a standalone single container such as the one started with *docker run*)
//
// the sizing information uses three values, cpu quota, cpu period and memory limit,
// and with above values it calculates the # vcpus and memory for the workload
//
// if the workload # of vcpus and memory is invalid for vmms, we still use default
// value in toml_config
#[derive(Clone, Copy, Debug)]
pub struct InitialSizeManager {
    resource: InitialSize,
}

impl InitialSizeManager {
    pub fn new(spec: &oci::Spec) -> Result<Self> {
        Ok(Self {
            resource: InitialSize::try_from(spec).context("failed to construct static resource")?,
        })
    }

    pub fn new_from(annotation: &HashMap<String, String>) -> Result<Self> {
        Ok(Self {
            resource: InitialSize::try_from(annotation)
                .context("failed to construct static resource")?,
        })
    }

    pub fn setup_config(&mut self, config: &mut TomlConfig) -> Result<()> {
        // update this data to the hypervisor config for later use by hypervisor
        let hypervisor_name = &config.runtime.hypervisor_name;
        let hv = config
            .hypervisor
            .get_mut(hypervisor_name)
            .context("failed to get hypervisor config")?;

        if self.resource.vcpu > 0 {
            hv.cpu_info.default_vcpus = self.resource.vcpu as i32
        }
        self.resource.orig_toml_default_mem = hv.memory_info.default_memory;
        if self.resource.mem_mb > 0 {
            // since the memory overhead introduced by kata-agent and system components
            // will really affect the amount of memory the user can use, so we choose to
            // plus the default_memory here, instead of overriding it.
            // (if we override the default_memory here, and user apllications still
            // use memory as they orignally expected, it would be easy to OOM.)
            hv.memory_info.default_memory += self.resource.mem_mb;
        }
        Ok(())
    }

    pub fn get_orig_toml_default_mem(&self) -> u32 {
        self.resource.orig_toml_default_mem
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
    use kata_types::annotations::cri_containerd;
    use oci_spec::runtime::{LinuxBuilder, LinuxMemory, LinuxMemoryBuilder, LinuxResourcesBuilder};
    use std::collections::HashMap;
    #[derive(Clone)]
    struct InputData {
        period: Option<u64>,
        quota: Option<i64>,
        memory: Option<i64>,
    }

    #[derive(Clone)]
    struct TestData<'a> {
        desc: &'a str,
        input: InputData,
        result: InitialSize,
    }

    fn get_test_data() -> Vec<TestData<'static>> {
        [
            TestData {
                desc: "no resource limit",
                input: InputData {
                    period: None,
                    quota: None,
                    memory: None,
                },
                result: InitialSize {
                    vcpu: 0,
                    mem_mb: 0,
                    orig_toml_default_mem: 0,
                },
            },
            TestData {
                desc: "normal resource limit",
                // data below should result in 2200 mCPU(round up to 3 vcpus) and 512 MiB of memory
                input: InputData {
                    period: Some(100_000),
                    quota: Some(220_000),
                    memory: Some(1024 * 1024 * 512),
                },
                result: InitialSize {
                    vcpu: 3,
                    mem_mb: 512,
                    orig_toml_default_mem: 0,
                },
            },
        ]
        .to_vec()
    }

    #[test]
    fn test_initial_size_sandbox() {
        let tests = get_test_data();

        // run tests
        for (i, d) in tests.iter().enumerate() {
            let mut spec = oci::Spec::default();
            spec.set_annotations(Some(HashMap::from([
                (
                    cri_containerd::CONTAINER_TYPE_LABEL_KEY.to_string(),
                    cri_containerd::SANDBOX.to_string(),
                ),
                (
                    cri_containerd::SANDBOX_CPU_PERIOD_KEY.to_string(),
                    d.input.period.map_or(String::new(), |v| format!("{}", v)),
                ), // CPU period
                (
                    cri_containerd::SANDBOX_CPU_QUOTA_KEY.to_string(),
                    d.input.quota.map_or(String::new(), |v| format!("{}", v)),
                ), // CPU quota
                (
                    cri_containerd::SANDBOX_MEM_KEY.to_string(),
                    d.input.memory.map_or(String::new(), |v| format!("{}", v)),
                ), // memory in bytes
            ])));

            let initial_size = InitialSize::try_from(&spec);
            assert!(
                initial_size.is_ok(),
                "test[{}]: {:?} should be ok",
                i,
                d.desc
            );

            let initial_size = initial_size.unwrap();
            assert_eq!(
                initial_size.vcpu, d.result.vcpu,
                "test[{}]: {:?} vcpu should be {}",
                i, d.desc, d.result.vcpu,
            );
            assert_eq!(
                initial_size.mem_mb, d.result.mem_mb,
                "test[{}]: {:?} memory should be {}",
                i, d.desc, d.result.mem_mb,
            );
        }
    }

    #[test]
    fn test_initial_size_container() {
        let tests = get_test_data();

        // run tests
        for (i, d) in tests.iter().enumerate() {
            let mut spec = oci::Spec::default();
            spec.set_annotations(Some(HashMap::from([(
                cri_containerd::CONTAINER_TYPE_LABEL_KEY.to_string(),
                cri_containerd::CONTAINER.to_string(),
            )])));

            let mut linux_cpu = oci::LinuxCpu::default();
            linux_cpu.set_period(d.input.period);
            linux_cpu.set_quota(d.input.quota);
            let mut linux_mem: LinuxMemory = LinuxMemory::default();
            if let Some(limit) = d.input.memory {
                linux_mem = LinuxMemoryBuilder::default().limit(limit).build().unwrap();
            };

            let linux = LinuxBuilder::default()
                .resources(
                    LinuxResourcesBuilder::default()
                        .cpu(linux_cpu)
                        .memory(linux_mem)
                        .build()
                        .unwrap(),
                )
                .build()
                .unwrap();
            spec.set_linux(Some(linux));

            let initial_size = InitialSize::try_from(&spec);
            assert!(
                initial_size.is_ok(),
                "test[{}]: {:?} should be ok",
                i,
                d.desc
            );

            let initial_size = initial_size.unwrap();
            assert_eq!(
                initial_size.vcpu, d.result.vcpu,
                "test[{}]: {:?} vcpu should be {}",
                i, d.desc, d.result.vcpu,
            );
            assert_eq!(
                initial_size.mem_mb, d.result.mem_mb,
                "test[{}]: {:?} memory should be {}",
                i, d.desc, d.result.mem_mb,
            );
        }
    }
}
