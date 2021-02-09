// Copyright (c) 2018 Levente Kurusa
// Copyright (c) 2020 Ant Group
//
// SPDX-License-Identifier: Apache-2.0 or MIT
//

//! This module allows the user to create a control group using the Builder pattern.
//! # Example
//!
//! The following example demonstrates how the control group builder looks like.  The user
//! specifies the name of the control group (here: "hello") and the hierarchy it belongs to (here:
//! a V1 hierarchy). Next, the user selects a subsystem by calling functions like `memory()`,
//! `cpu()` and `devices()`. The user can then add restrictions and details via subsystem-specific
//! calls. To finalize a subsystem, the user may call `done()`. Finally, if the control group build
//! is done and all requirements/restrictions have been specified, the control group can be created
//! by a call to `build()`.
//!
//! ```rust,no_run
//! # use cgroups::*;
//! # use cgroups::devices::*;
//! # use cgroups::cgroup_builder::*;
//! let h = cgroups::hierarchies::auto();
//! let h = Box::new(&*h);
//! let cgroup: Cgroup = CgroupBuilder::new("hello", h)
//!      .memory()
//!          .kernel_memory_limit(1024 * 1024)
//!          .memory_hard_limit(1024 * 1024)
//!          .done()
//!      .cpu()
//!          .shares(100)
//!          .done()
//!      .devices()
//!          .device(1000, 10, DeviceType::Block, true,
//!             vec![DevicePermissions::Read,
//!                  DevicePermissions::Write,
//!                  DevicePermissions::MkNod])
//!          .device(6, 1, DeviceType::Char, false, vec![])
//!          .done()
//!      .network()
//!          .class_id(1337)
//!          .priority("eth0".to_string(), 100)
//!          .priority("wl0".to_string(), 200)
//!          .done()
//!      .hugepages()
//!          .limit("2M".to_string(), 0)
//!          .limit("4M".to_string(), 4 * 1024 * 1024 * 100)
//!          .limit("2G".to_string(), 2 * 1024 * 1024 * 1024)
//!          .done()
//!      .blkio()
//!          .weight(Some(123))
//!          .leaf_weight(Some(99))
//!          .weight_device(6, 1, Some(100), Some(55))
//!          .weight_device(6, 1, Some(100), Some(55))
//!          .throttle_iops()
//!              .read(6, 1, 10)
//!              .write(11, 1, 100)
//!          .throttle_bps()
//!              .read(6, 1, 10)
//!              .write(11, 1, 100)
//!          .done()
//!      .build();
//! ```
use crate::error::*;

use crate::{
    pid, BlkIoDeviceResource, BlkIoDeviceThrottleResource, Cgroup, DeviceResource, Hierarchy,
    HugePageResource, MaxValue, NetworkPriority, Resources,
};

macro_rules! gen_setter {
    ($res:ident, $cont:ident, $func:ident, $name:ident, $ty:ty) => {
        /// See the similarly named function in the respective controller.
        pub fn $name(mut self, $name: $ty) -> Self {
            self.cgroup.resources.$res.update_values = true;
            self.cgroup.resources.$res.$name = $name;
            self
        }
    };
}

/// A control group builder instance
pub struct CgroupBuilder<'a> {
    name: String,
    hierarchy: Box<&'a dyn Hierarchy>,
    /// Internal, unsupported field: use the associated builders instead.
    resources: Resources,
}

impl<'a> CgroupBuilder<'a> {
    /// Start building a control group with the supplied hierarchy and name pair.
    ///
    /// Note that this does not actually create the control group until `build()` is called.
    pub fn new(name: &'a str, hierarchy: Box<&'a dyn Hierarchy>) -> CgroupBuilder<'a> {
        CgroupBuilder {
            name: name.to_owned(),
            hierarchy: hierarchy,
            resources: Resources::default(),
        }
    }

    /// Builds the memory resources of the control group.
    pub fn memory(self) -> MemoryResourceBuilder<'a> {
        MemoryResourceBuilder { cgroup: self }
    }

    /// Builds the pid resources of the control group.
    pub fn pid(self) -> PidResourceBuilder<'a> {
        PidResourceBuilder { cgroup: self }
    }

    /// Builds the cpu resources of the control group.
    pub fn cpu(self) -> CpuResourceBuilder<'a> {
        CpuResourceBuilder { cgroup: self }
    }

    /// Builds the devices resources of the control group, disallowing or
    /// allowing access to certain devices in the system.
    pub fn devices(self) -> DeviceResourceBuilder<'a> {
        DeviceResourceBuilder { cgroup: self }
    }

    /// Builds the network resources of the control group, setting class id, or
    /// various priorities on networking interfaces.
    pub fn network(self) -> NetworkResourceBuilder<'a> {
        NetworkResourceBuilder { cgroup: self }
    }

    /// Builds the hugepage/hugetlb resources available to the control group.
    pub fn hugepages(self) -> HugepagesResourceBuilder<'a> {
        HugepagesResourceBuilder { cgroup: self }
    }

    /// Builds the block I/O resources available for the control group.
    pub fn blkio(self) -> BlkIoResourcesBuilder<'a> {
        BlkIoResourcesBuilder {
            cgroup: self,
            throttling_iops: false,
        }
    }

    /// Finalize the control group, consuming the builder and creating the control group.
    pub fn build(self) -> Cgroup<'a> {
        let cg = Cgroup::new(self.hierarchy, self.name);
        cg.apply(&self.resources);
        cg
    }
}

/// A builder that configures the memory controller of a control group.
pub struct MemoryResourceBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
}

impl<'a> MemoryResourceBuilder<'a> {
    gen_setter!(
        memory,
        MemController,
        set_kmem_limit,
        kernel_memory_limit,
        i64
    );
    gen_setter!(memory, MemController, set_limit, memory_hard_limit, i64);
    gen_setter!(
        memory,
        MemController,
        set_soft_limit,
        memory_soft_limit,
        i64
    );
    gen_setter!(
        memory,
        MemController,
        set_tcp_limit,
        kernel_tcp_memory_limit,
        i64
    );
    gen_setter!(
        memory,
        MemController,
        set_memswap_limit,
        memory_swap_limit,
        i64
    );
    gen_setter!(memory, MemController, set_swappiness, swappiness, u64);

    /// Finish the construction of the memory resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}

/// A builder that configures the pid controller of a control group.
pub struct PidResourceBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
}

impl<'a> PidResourceBuilder<'a> {
    gen_setter!(
        pid,
        PidController,
        set_pid_max,
        maximum_number_of_processes,
        MaxValue
    );

    /// Finish the construction of the pid resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}

/// A builder that configures the cpuset & cpu controllers of a control group.
pub struct CpuResourceBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
}

impl<'a> CpuResourceBuilder<'a> {
    // FIXME this should all changed to options.
    gen_setter!(cpu, CpuSetController, set_cpus, cpus, Option<String>);
    gen_setter!(cpu, CpuSetController, set_mems, mems, String);
    gen_setter!(cpu, CpuController, set_shares, shares, u64);
    gen_setter!(cpu, CpuController, set_cfs_quota, quota, i64);
    gen_setter!(cpu, CpuController, set_cfs_period, period, u64);
    gen_setter!(cpu, CpuController, set_rt_runtime, realtime_runtime, i64);
    gen_setter!(cpu, CpuController, set_rt_period, realtime_period, u64);

    /// Finish the construction of the cpu resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}

/// A builder that configures the devices controller of a control group.
pub struct DeviceResourceBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
}

impl<'a> DeviceResourceBuilder<'a> {
    /// Restrict (or allow) a device to the tasks inside the control group.
    pub fn device(
        mut self,
        major: i64,
        minor: i64,
        devtype: crate::devices::DeviceType,
        allow: bool,
        access: Vec<crate::devices::DevicePermissions>,
    ) -> DeviceResourceBuilder<'a> {
        self.cgroup.resources.devices.update_values = true;
        self.cgroup.resources.devices.devices.push(DeviceResource {
            major,
            minor,
            devtype,
            allow,
            access,
        });
        self
    }

    /// Finish the construction of the devices resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}

/// A builder that configures the net_cls & net_prio controllers of a control group.
pub struct NetworkResourceBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
}

impl<'a> NetworkResourceBuilder<'a> {
    gen_setter!(network, NetclsController, set_class, class_id, u64);

    /// Set the priority of the tasks when operating on a networking device defined by `name` to be
    /// `priority`.
    pub fn priority(mut self, name: String, priority: u64) -> NetworkResourceBuilder<'a> {
        self.cgroup.resources.network.update_values = true;
        self.cgroup
            .resources
            .network
            .priorities
            .push(NetworkPriority { name, priority });
        self
    }

    /// Finish the construction of the network resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}

/// A builder that configures the hugepages controller of a control group.
pub struct HugepagesResourceBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
}

impl<'a> HugepagesResourceBuilder<'a> {
    /// Limit the usage of certain hugepages (determined by `size`) to be at most `limit` bytes.
    pub fn limit(mut self, size: String, limit: u64) -> HugepagesResourceBuilder<'a> {
        self.cgroup.resources.hugepages.update_values = true;
        self.cgroup
            .resources
            .hugepages
            .limits
            .push(HugePageResource { size, limit });
        self
    }

    /// Finish the construction of the network resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}

/// A builder that configures the blkio controller of a control group.
pub struct BlkIoResourcesBuilder<'a> {
    cgroup: CgroupBuilder<'a>,
    throttling_iops: bool,
}

impl<'a> BlkIoResourcesBuilder<'a> {
    gen_setter!(blkio, BlkIoController, set_weight, weight, Option<u16>);
    gen_setter!(
        blkio,
        BlkIoController,
        set_leaf_weight,
        leaf_weight,
        Option<u16>
    );

    /// Set the weight of a certain device.
    pub fn weight_device(
        mut self,
        major: u64,
        minor: u64,
        weight: Option<u16>,
        leaf_weight: Option<u16>,
    ) -> BlkIoResourcesBuilder<'a> {
        self.cgroup.resources.blkio.update_values = true;
        self.cgroup
            .resources
            .blkio
            .weight_device
            .push(BlkIoDeviceResource {
                major,
                minor,
                weight,
                leaf_weight,
            });
        self
    }

    /// Start configuring the I/O operations per second metric.
    pub fn throttle_iops(mut self) -> BlkIoResourcesBuilder<'a> {
        self.throttling_iops = true;
        self
    }

    /// Start configuring the bytes per second metric.
    pub fn throttle_bps(mut self) -> BlkIoResourcesBuilder<'a> {
        self.throttling_iops = false;
        self
    }

    /// Limit the read rate of the current metric for a certain device.
    pub fn read(mut self, major: u64, minor: u64, rate: u64) -> BlkIoResourcesBuilder<'a> {
        self.cgroup.resources.blkio.update_values = true;
        let throttle = BlkIoDeviceThrottleResource { major, minor, rate };
        if self.throttling_iops {
            self.cgroup
                .resources
                .blkio
                .throttle_read_iops_device
                .push(throttle);
        } else {
            self.cgroup
                .resources
                .blkio
                .throttle_read_bps_device
                .push(throttle);
        }
        self
    }

    /// Limit the write rate of the current metric for a certain device.
    pub fn write(mut self, major: u64, minor: u64, rate: u64) -> BlkIoResourcesBuilder<'a> {
        self.cgroup.resources.blkio.update_values = true;
        let throttle = BlkIoDeviceThrottleResource { major, minor, rate };
        if self.throttling_iops {
            self.cgroup
                .resources
                .blkio
                .throttle_write_iops_device
                .push(throttle);
        } else {
            self.cgroup
                .resources
                .blkio
                .throttle_write_bps_device
                .push(throttle);
        }
        self
    }

    /// Finish the construction of the blkio resources of a control group.
    pub fn done(self) -> CgroupBuilder<'a> {
        self.cgroup
    }
}
