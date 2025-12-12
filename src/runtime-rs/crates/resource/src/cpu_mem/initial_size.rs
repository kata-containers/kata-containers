// Copyright (c) 2019-2021 Alibaba Cloud
// Copyright (c) 2019-2021 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use std::{collections::HashMap, convert::TryFrom};

use anyhow::{Context, Result};
use kata_types::{
    annotations::Annotation, config::TomlConfig, container::ContainerType, cpu::CpuSet,
    k8s::container_type,
};
use oci_spec::runtime as oci;

// initial resource that InitialSizeManager needs, this is the spec for the
// sandbox/container's workload
#[derive(Clone, Copy, Debug)]
struct InitialSize {
    vcpu: f32,
    mem_mb: u32,
    orig_toml_default_mem: u32,
}

const MIB: i64 = 1024 * 1024;
const SHARES_PER_CPU: u64 = 1024;

fn initial_size_from_sandbox_annotation(
    annotation: &Annotation,
    cpu_set: Option<&str>,
) -> InitialSize {
    // Since we are *adding* our result to the config, a value of 0 will cause no change.
    // If the annotation is not assigned (but static resource management is), upper layers
    // may still provide CPUSet/CPU shares via the spec.
    let period = annotation.get_sandbox_cpu_period();
    let quota = annotation.get_sandbox_cpu_quota();
    let shares = annotation.get_sandbox_cpu_shares();
    let vcpu = calculate_vcpus(period, quota, shares, cpu_set);
    let mem_mb = convert_memory_to_mb(annotation.get_sandbox_mem());

    InitialSize {
        vcpu,
        mem_mb,
        orig_toml_default_mem: 0,
    }
}

// generate initial resource(vcpu and memory in MiB) from annotations
impl TryFrom<&HashMap<String, String>> for InitialSize {
    type Error = anyhow::Error;
    fn try_from(an: &HashMap<String, String>) -> Result<Self> {
        let annotation = Annotation::new(an.clone());
        Ok(initial_size_from_sandbox_annotation(&annotation, None))
    }
}

// generate initial resource(vcpu and memory in MiB) from spec's information
impl TryFrom<&oci::Spec> for InitialSize {
    type Error = anyhow::Error;
    fn try_from(spec: &oci::Spec) -> Result<Self> {
        let initial_size = match container_type(spec) {
            // podsandbox, from annotation
            ContainerType::PodSandbox => {
                let spec_annos = spec.annotations().clone().unwrap_or_default();
                let annotation = Annotation::new(spec_annos);
                let cpu_set = spec
                    .linux()
                    .as_ref()
                    .and_then(|linux| linux.resources().as_ref())
                    .and_then(|resources| resources.cpu().as_ref())
                    .and_then(|cpu| cpu.cpus().as_deref())
                    .filter(|s| !s.is_empty());

                initial_size_from_sandbox_annotation(&annotation, cpu_set)
            }
            // single container, from container spec
            _ => {
                let resource = spec
                    .linux()
                    .as_ref()
                    .and_then(|linux| linux.resources().as_ref());

                let vcpu = resource
                    .and_then(|r| r.cpu().as_ref())
                    .map(|cpu| {
                        let period = cpu.period().unwrap_or(0);
                        let quota = cpu.quota().unwrap_or(-1);
                        let shares = cpu.shares().unwrap_or(0);
                        let cpu_set = cpu.cpus().as_deref().filter(|s| !s.is_empty());

                        calculate_vcpus(period, quota, shares, cpu_set)
                    })
                    .unwrap_or(0.0);

                let mem_mb = resource
                    .and_then(|r| r.memory().as_ref())
                    .and_then(|mem| mem.limit())
                    .map(convert_memory_to_mb)
                    .unwrap_or(0);

                InitialSize {
                    vcpu,
                    mem_mb,
                    orig_toml_default_mem: 0,
                }
            }
        };
        info!(
            sl!(),
            "(from PodSandbox's annotation / SingleContainer's spec) initial size: vcpu={}, mem_mb={}",
            initial_size.vcpu,
            initial_size.mem_mb
        );
        Ok(initial_size)
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

        if self.resource.vcpu > 0.0 {
            info!(sl!(), "resource with vcpu {}", self.resource.vcpu);
            if config.runtime.static_sandbox_resource_mgmt {
                hv.cpu_info.default_vcpus += self.resource.vcpu;
                // Ensure the hypervisor max vCPU limit can accommodate the new boot vCPU count,
                // but do not shrink an existing higher default_maxvcpus (and preserve 0 as
                // a hypervisor-specific "use default" value).
                let boot_vcpus = hv.cpu_info.default_vcpus.ceil() as u32;
                if hv.cpu_info.default_maxvcpus != 0 {
                    hv.cpu_info.default_maxvcpus = hv.cpu_info.default_maxvcpus.max(boot_vcpus);
                }
            }
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

fn calculate_vcpus(period: u64, quota: i64, shares: u64, cpu_set: Option<&str>) -> f32 {
    if quota >= 0 && period > 0 {
        return quota as f32 / period as f32;
    }

    // If quota is unconstrained and shares are provided, use shares as an approximation
    // of the requested CPUs (1024 shares per CPU in Kubernetes). This avoids sizing the
    // VM to the whole CPUSet for best-effort/burstable cases.
    if shares > 0 {
        return if shares >= SHARES_PER_CPU {
            shares as f32 / SHARES_PER_CPU as f32
        } else {
            0.0
        };
    }

    cpu_set
        .and_then(|s| s.parse::<CpuSet>().ok())
        .map(|set| set.len() as f32)
        .unwrap_or(0.0)
}

fn convert_memory_to_mb(memory_in_byte: i64) -> u32 {
    if memory_in_byte < 0 {
        return 0;
    }
    let mem_size = (memory_in_byte / MIB) as u32;
    // memory size must be 2MB aligned for hugepage support
    if mem_size % 2 != 0 {
        return mem_size + 1;
    }

    mem_size
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::annotations::cri_containerd;
    use kata_types::config::hypervisor::Hypervisor;
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
                    vcpu: 0.0,
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
                    memory: Some(512 * MIB),
                },
                result: InitialSize {
                    vcpu: 3.0,
                    mem_mb: 512,
                    orig_toml_default_mem: 0,
                },
            },
            TestData {
                desc: "Odd memory in resource limits",
                input: InputData {
                    period: None,
                    quota: None,
                    memory: Some(513 * MIB),
                },
                result: InitialSize {
                    vcpu: 0.0,
                    mem_mb: 514,
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
                initial_size.vcpu.ceil(),
                d.result.vcpu,
                "test[{}]: {:?} vcpu should be {}",
                i,
                d.desc,
                d.result.vcpu,
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
                initial_size.vcpu.ceil(),
                d.result.vcpu,
                "test[{}]: {:?} vcpu should be {}",
                i,
                d.desc,
                d.result.vcpu,
            );
            assert_eq!(
                initial_size.mem_mb, d.result.mem_mb,
                "test[{}]: {:?} memory should be {}",
                i, d.desc, d.result.mem_mb,
            );
        }
    }

    #[test]
    fn test_initial_size_sandbox_uses_shares_when_quota_unconstrained() {
        let mut spec = oci::Spec::default();
        spec.set_annotations(Some(HashMap::from([
            (
                cri_containerd::CONTAINER_TYPE_LABEL_KEY.to_string(),
                cri_containerd::SANDBOX.to_string(),
            ),
            (
                cri_containerd::SANDBOX_CPU_PERIOD_KEY.to_string(),
                "100000".to_string(),
            ),
            (
                cri_containerd::SANDBOX_CPU_QUOTA_KEY.to_string(),
                "-1".to_string(),
            ),
            (
                cri_containerd::SANDBOX_CPU_SHARE_KEY.to_string(),
                "51200".to_string(),
            ),
            (
                cri_containerd::SANDBOX_MEM_KEY.to_string(),
                format!("{}", 512 * MIB),
            ),
        ])));

        let initial_size = InitialSize::try_from(&spec).unwrap();
        assert_eq!(initial_size.vcpu.ceil(), 50.0);
        assert_eq!(initial_size.mem_mb, 512);
    }

    #[test]
    fn test_initial_size_container_uses_shares_and_ignores_cpuset_for_best_effort() {
        let mut spec = oci::Spec::default();
        spec.set_annotations(Some(HashMap::from([(
            cri_containerd::CONTAINER_TYPE_LABEL_KEY.to_string(),
            cri_containerd::CONTAINER.to_string(),
        )])));

        let mut linux_cpu = oci::LinuxCpu::default();
        linux_cpu.set_quota(Some(-1));
        linux_cpu.set_period(Some(100000));
        linux_cpu.set_shares(Some(2));
        linux_cpu.set_cpus(Some("0-63".to_string()));

        let linux = LinuxBuilder::default()
            .resources(
                LinuxResourcesBuilder::default()
                    .cpu(linux_cpu)
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();
        spec.set_linux(Some(linux));

        let initial_size = InitialSize::try_from(&spec).unwrap();
        assert_eq!(initial_size.vcpu.ceil(), 0.0);
    }

    #[test]
    fn test_setup_config_updates_boot_vcpus_for_static_resource_mgmt() {
        let mut config = TomlConfig::default();
        config.runtime.hypervisor_name = "qemu".to_string();
        config.runtime.static_sandbox_resource_mgmt = true;

        let mut hv = Hypervisor::default();
        hv.cpu_info.default_vcpus = 1.0;
        hv.cpu_info.default_maxvcpus = 1;
        hv.memory_info.default_memory = 2048;
        config.hypervisor.insert("qemu".to_string(), hv);

        let annos = HashMap::from([
            (
                cri_containerd::SANDBOX_CPU_SHARE_KEY.to_string(),
                "2048".to_string(),
            ),
            (
                cri_containerd::SANDBOX_MEM_KEY.to_string(),
                format!("{}", 512 * MIB),
            ),
        ]);

        let mut mgr = InitialSizeManager::new_from(&annos).unwrap();
        mgr.setup_config(&mut config).unwrap();

        let hv = config.hypervisor.get("qemu").unwrap();
        assert_eq!(hv.cpu_info.default_vcpus.ceil(), 3.0);
        assert_eq!(hv.cpu_info.default_maxvcpus, 3);
        assert_eq!(hv.memory_info.default_memory, 2048 + 512);
    }

    #[test]
    fn test_setup_config_does_not_reduce_default_maxvcpus() {
        let mut config = TomlConfig::default();
        config.runtime.hypervisor_name = "qemu".to_string();
        config.runtime.static_sandbox_resource_mgmt = true;

        let mut hv = Hypervisor::default();
        hv.cpu_info.default_vcpus = 1.0;
        hv.cpu_info.default_maxvcpus = 240;
        hv.memory_info.default_memory = 2048;
        config.hypervisor.insert("qemu".to_string(), hv);

        let annos = HashMap::from([(
            cri_containerd::SANDBOX_CPU_SHARE_KEY.to_string(),
            "2048".to_string(),
        )]);

        let mut mgr = InitialSizeManager::new_from(&annos).unwrap();
        mgr.setup_config(&mut config).unwrap();

        let hv = config.hypervisor.get("qemu").unwrap();
        assert_eq!(hv.cpu_info.default_vcpus.ceil(), 3.0);
        assert_eq!(hv.cpu_info.default_maxvcpus, 240);
    }
}
