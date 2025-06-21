// Copyright (c) 2019-2022 Alibaba Cloud
// Copyright (c) 2019-2022 Ant Group
//
// SPDX-License-Identifier: Apache-2.0
//

use anyhow::{Context, Ok, Result};
use hypervisor::Hypervisor;
use kata_types::{config::TomlConfig, cpu::LinuxContainerCpuResources};
use oci::LinuxCpu;
use oci_spec::runtime as oci;
use std::{
    cmp,
    collections::{HashMap, HashSet},
    convert::TryFrom,
    sync::Arc,
};
use tokio::sync::RwLock;

use crate::ResourceUpdateOp;

#[derive(Default, Debug, Clone)]
pub struct CpuResource {
    /// Current number of vCPUs
    pub(crate) current_vcpu: Arc<RwLock<u32>>,

    /// Default number of vCPUs
    pub(crate) default_vcpu: f32,

    /// CpuResource of each container
    pub(crate) container_cpu_resources: Arc<RwLock<HashMap<String, LinuxContainerCpuResources>>>,
}

impl CpuResource {
    pub fn new(config: Arc<TomlConfig>) -> Result<Self> {
        let hypervisor_name = config.runtime.hypervisor_name.clone();
        let hypervisor_config = config
            .hypervisor
            .get(&hypervisor_name)
            .context(format!("failed to get hypervisor {}", hypervisor_name))?;
        Ok(Self {
            current_vcpu: Arc::new(RwLock::new(hypervisor_config.cpu_info.default_vcpus as u32)),
            default_vcpu: hypervisor_config.cpu_info.default_vcpus,
            container_cpu_resources: Arc::new(RwLock::new(HashMap::new())),
        })
    }

    pub(crate) async fn update_cpu_resources(
        &self,
        cid: &str,
        linux_cpus: Option<&LinuxCpu>,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<()> {
        self.update_container_cpu_resources(cid, linux_cpus, op)
            .await
            .context("update container cpu resources")?;
        let vcpu_required = self
            .calc_cpu_resources()
            .await
            .context("calculate vcpus required")?;

        if vcpu_required == self.current_vcpu().await {
            return Ok(());
        }

        let curr_vcpus = self
            .do_update_cpu_resources(vcpu_required, op, hypervisor)
            .await?;
        self.update_current_vcpu(curr_vcpus).await;
        Ok(())
    }

    pub(crate) async fn current_vcpu(&self) -> u32 {
        let current_vcpu = self.current_vcpu.read().await;
        *current_vcpu
    }

    async fn update_current_vcpu(&self, new_vcpus: u32) {
        let mut current_vcpu = self.current_vcpu.write().await;
        *current_vcpu = new_vcpus;
    }

    // update container_cpu_resources field
    async fn update_container_cpu_resources(
        &self,
        cid: &str,
        linux_cpus: Option<&LinuxCpu>,
        op: ResourceUpdateOp,
    ) -> Result<()> {
        if let Some(cpu) = linux_cpus {
            let container_resource = LinuxContainerCpuResources::try_from(cpu)?;
            let mut resources = self.container_cpu_resources.write().await;
            match op {
                ResourceUpdateOp::Add => {
                    resources.insert(cid.to_owned(), container_resource);
                }
                ResourceUpdateOp::Update => {
                    let resource = resources.insert(cid.to_owned(), container_resource.clone());
                    if let Some(old_container_resource) = resource {
                        // the priority of cpu-quota is higher than cpuset when determine the number of vcpus.
                        // we should better ignore the resource update when update cpu only by cpuset if cpu-quota
                        // has been set previously.
                        if old_container_resource.quota() > 0 && container_resource.quota() < 0 {
                            resources.insert(cid.to_owned(), old_container_resource);
                        }
                    }
                }
                ResourceUpdateOp::Del => {
                    resources.remove(cid);
                }
            }
        }

        Ok(())
    }

    // calculates the total required vcpus by adding each container's requirements within the pod
    async fn calc_cpu_resources(&self) -> Result<u32> {
        let resources = self.container_cpu_resources.read().await;
        if resources.is_empty() {
            return Ok(self.default_vcpu.ceil() as u32);
        }

        // If requests of individual containers are expresses with different
        // periods we'll need to rewrite them with a common denominator
        // (period) before we can add the numerators (quotas).  We choose
        // to use the largest period as the common denominator since it
        // shifts precision out of the fractional part and into the
        // integral part in case a rewritten quota ends up non-integral.
        let max_period = resources
            .iter()
            .map(|(_, cpu_resource)| cpu_resource.period())
            .max()
            // It's ok to unwrap() here as we have checked that 'resources' is
            // not empty.
            .unwrap() as f64;

        let mut cpuset_vcpu: HashSet<u32> = HashSet::new();
        // Even though summing up quotas is fixed-point conceptually we
        // represent the sum as floating-point because
        // - we might be rewriting the quota/period fractions if periods
        //   vary,and a rewritten quota can end up non-integral.  We want
        //   to preserve the fractional parts until the final rounding
        //   not to lose precision inadvertenty.
        // - also to avoid some tedious casting doing maths with quotas.
        // Using a 64-bit float to represent what are conceptually integral
        // numbers should be safe here - f64 starts losing precision for
        // integers only past 2^53 and a sums of quotas are extremely unlikely
        // to reach that magnitude.
        let mut total_quota: f64 = 0.0;

        for (_, cpu_resource) in resources.iter() {
            cpuset_vcpu.extend(cpu_resource.cpuset().iter());

            let quota = cpu_resource.quota() as f64;
            let period = cpu_resource.period() as f64;
            if quota >= 0.0 && period != 0.0 {
                total_quota += quota * (max_period / period);
            }
        }

        // contrained only by cpuset
        if total_quota == 0.0 && !cpuset_vcpu.is_empty() {
            info!(sl!(), "(from cpuset)get vcpus # {:?}", cpuset_vcpu);
            return Ok(cpuset_vcpu.len() as u32);
        }

        let total_vcpu = if total_quota > 0.0 && max_period != 0.0 {
            self.default_vcpu as f64 + total_quota / max_period
        } else {
            self.default_vcpu as f64
        };

        info!(
            sl!(),
            "(from cfs_quota&cfs_period)get vcpus count {}", total_vcpu
        );
        Ok(total_vcpu.ceil() as u32)
    }

    // do hotplug and hot-unplug the vcpu
    async fn do_update_cpu_resources(
        &self,
        new_vcpus: u32,
        op: ResourceUpdateOp,
        hypervisor: &dyn Hypervisor,
    ) -> Result<u32> {
        let old_vcpus = self.current_vcpu().await;

        // when adding vcpus, ignore old_vcpus > new_vcpus
        // when deleting vcpus, ignore old_vcpus < new_vcpus
        if (op == ResourceUpdateOp::Add && old_vcpus > new_vcpus)
            || (op == ResourceUpdateOp::Del && old_vcpus < new_vcpus)
        {
            return Ok(old_vcpus);
        }

        // do not reduce computing power
        // the number of vcpus would not be lower than the default size
        let new_vcpus = cmp::max(new_vcpus, self.default_vcpu.ceil() as u32);

        let (_, new) = hypervisor
            .resize_vcpu(old_vcpus, new_vcpus)
            .await
            .context("resize vcpus")?;

        Ok(new)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kata_types::config::{Hypervisor, TomlConfig};
    use oci::LinuxCpu;

    fn get_cpu_resource_with_default_vcpus(default_vcpus: f32) -> CpuResource {
        let mut config = TomlConfig::default();
        config
            .hypervisor
            .insert("qemu".to_owned(), Hypervisor::default());
        config
            .hypervisor
            .entry("qemu".to_owned())
            .and_modify(|hv_config| hv_config.cpu_info.default_vcpus = default_vcpus);
        config.runtime.hypervisor_name = "qemu".to_owned();
        CpuResource::new(Arc::new(config)).unwrap()
    }

    async fn add_linux_container_cpu_resources(cpu_res: &mut CpuResource, res: Vec<(i64, u64)>) {
        let mut resources = cpu_res.container_cpu_resources.write().await;
        for (i, (quota, period)) in res.iter().enumerate() {
            let mut linux_cpu = LinuxCpu::default();
            linux_cpu.set_quota(Some(*quota));
            linux_cpu.set_period(Some(*period));
            let res = LinuxContainerCpuResources::try_from(&linux_cpu).unwrap();
            resources.insert(i.to_string(), res);
        }
    }

    // A lot of the following tests document why a fixed-point-style
    // calc_cpu_resources() implementation is better than a f32-based one.
    #[tokio::test]
    async fn test_rounding() {
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(0.0);

        // A f32-based calc_cpu_resources() implementation would fail this
        // test (adding 0.1 ten times gives roughly 1.0000001).
        // An f64-based implementation would pass this one (with the summation
        // result of 0.99999999999999989) but it still doesn't guarantee the
        // correct result in general.  For instance, adding 0.1 twenty times
        // in 64 bits results in 2.0000000000000004.
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
                (100_000, 1_000_000),
            ],
        )
        .await;

        assert_eq!(cpu_resource.calc_cpu_resources().await.unwrap(), 1);
    }

    #[tokio::test]
    async fn test_big_allocation_1() {
        let default_vcpus = 10.0;

        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![
                (32_000_000, 1_000_000),
                (32_000_000, 1_000_000),
                (64_000_000, 1_000_000),
            ],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            128 + default_vcpus as u32
        );
    }

    #[tokio::test]
    async fn test_big_allocation_2() {
        let default_vcpus = 10.0;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![
                (33_000_000, 1_000_000),
                (31_000_000, 1_000_000),
                (77_000_011, 1_000_000),
            ],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            (33 + 31 + 77 + 1) + default_vcpus as u32
        );
    }

    #[tokio::test]
    async fn test_big_allocation_3() {
        let default_vcpus = 10.0;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(&mut cpu_resource, vec![(141_000_008, 1_000_000)]).await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            142 + default_vcpus as u32
        );
    }

    #[tokio::test]
    async fn test_big_allocation_4() {
        let default_vcpus = 10.0;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![
                (17_000_001, 1_000_000),
                (17_000_001, 1_000_000),
                (17_000_001, 1_000_000),
                (17_000_001, 1_000_000),
            ],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            (4 * 17 + 1) + default_vcpus as u32
        );
    }

    #[tokio::test]
    async fn test_divisible_periods() {
        let default_vcpus = 3.0;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![(1_000_000, 1_000_000), (1_000_000, 500_000)],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            3 + default_vcpus as u32
        );

        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![(3_000_000, 1_500_000), (1_000_000, 500_000)],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            4 + default_vcpus as u32
        );
    }

    #[tokio::test]
    async fn test_indivisible_periods() {
        let default_vcpus = 1.0;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![(1_000_000, 1_000_000), (900_000, 300_000)],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            4 + default_vcpus as u32
        );

        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![(1_000_000, 1_000_000), (900_000, 299_999)],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            5 + default_vcpus as u32
        );
    }

    #[tokio::test]
    async fn test_fractional_default_vcpus() {
        let default_vcpus = 0.5;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(&mut cpu_resource, vec![(250_000, 1_000_000)]).await;

        assert_eq!(cpu_resource.calc_cpu_resources().await.unwrap(), 1);

        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(&mut cpu_resource, vec![(500_000, 1_000_000)]).await;

        assert_eq!(cpu_resource.calc_cpu_resources().await.unwrap(), 1);

        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(&mut cpu_resource, vec![(500_001, 1_000_000)]).await;

        assert_eq!(cpu_resource.calc_cpu_resources().await.unwrap(), 2);

        // This test doesn't pass because 0.1 is periodic in binary and thus
        // not exactly representable by a float of any width for fundamental
        // reasons.  Its actual representation is slightly over 0.1
        // (e.g. 0.100000001 in f32), which after adding the 900_000/1_000_000
        // container request pushes the sum over 1.
        // I don't think this problem is solvable without expressing
        // 'default_vcpus' in configuration.toml in a fixed point manner (e.g.
        // as an integral percentage of a vCPU).
        /*
        let default_vcpus = 0.1;
        let mut cpu_resource = get_cpu_resource_with_default_vcpus(default_vcpus);
        add_linux_container_cpu_resources(
            &mut cpu_resource,
            vec![(900_000, 1_000_000)],
        )
        .await;

        assert_eq!(
            cpu_resource.calc_cpu_resources().await.unwrap(),
            1
        );
        */
    }
}
