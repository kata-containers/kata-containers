// Copyright (C) 2022 Alibaba Cloud. All rights reserved.
// Copyright 2018 Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

#[cfg(target_arch = "x86_64")]
use dbs_legacy_devices::I8042DeviceMetrics;
#[cfg(target_arch = "aarch64")]
use dbs_legacy_devices::RTCDeviceMetrics;
use dbs_legacy_devices::SerialDeviceMetrics;
#[cfg(feature = "dbs-upcall")]
use dbs_upcall::UpcallClientMetrics;
use dbs_utils::metric::SharedIncMetric;
#[cfg(feature = "virtio-balloon")]
use dbs_virtio_devices::balloon::BalloonDeviceMetrics;
#[cfg(feature = "virtio-blk")]
use dbs_virtio_devices::block::BlockDeviceMetrics;
#[cfg(feature = "virtio-fs")]
use dbs_virtio_devices::fs::FsDeviceMetrics;
use dbs_virtio_devices::mmio::MMIODeviceMetrics;
#[cfg(feature = "virtio-net")]
use dbs_virtio_devices::net::NetDeviceMetrics;
#[cfg(feature = "virtio-vsock")]
use dbs_virtio_devices::vsock::VsockDeviceMetrics;
use lazy_static::lazy_static;
use serde::Serialize;

lazy_static! {
    /// # Static instance used for handling metrics.
    ///
    /// Using a big lock over the DragonballMetrics since we have various device metric types
    /// and the write operation is only used when creating or removing devices, it has a low
    /// competitive overhead.
    pub static ref METRICS: RwLock<DragonballMetrics> = RwLock::new(DragonballMetrics::default());
}

/// Metrics specific to VCPUs' mode of functioning.
#[derive(Default, Serialize)]
pub struct VcpuMetrics {
    /// Number of KVM exits for handling input IO.
    pub exit_io_in: SharedIncMetric,
    /// Number of KVM exits for handling output IO.
    pub exit_io_out: SharedIncMetric,
    /// Number of KVM exits for handling MMIO reads.
    pub exit_mmio_read: SharedIncMetric,
    /// Number of KVM exits for handling MMIO writes.
    pub exit_mmio_write: SharedIncMetric,
    /// Number of errors during this VCPU's run.
    pub failures: SharedIncMetric,
    /// Failures in configuring the CPUID.
    pub filter_cpuid: SharedIncMetric,
}

/// Metrics for the seccomp filtering.
#[derive(Default, Serialize)]
pub struct SeccompMetrics {
    /// Number of errors inside the seccomp filtering.
    pub num_faults: SharedIncMetric,
}

/// Metrics related to signals.
#[derive(Default, Serialize)]
pub struct SignalMetrics {
    /// Number of times that SIGBUS was handled.
    pub sigbus: SharedIncMetric,
    /// Number of times that SIGSEGV was handled.
    pub sigsegv: SharedIncMetric,
}

/// Metrics related to vmm action request.
#[derive(Default, Serialize)]
pub struct VmmRequestMetrics {
    /// Number of vmm action request.
    pub request_count: SharedIncMetric,
    ///  Number of errors when sending the request.
    pub request_fails: SharedIncMetric,
    /// Number of errors received from the request.
    pub response_fails: SharedIncMetric,
    /// Number of times when configuring boot source.
    pub configure_boot_source: SharedIncMetric,
    /// Number of start vm operation.
    pub start_microvm: SharedIncMetric,
    /// Number of shutdown vm operation.
    pub shutdown_microvm: SharedIncMetric,
    /// Number of times when getting vm configuration.
    pub get_vm_configuration: SharedIncMetric,
    /// Number of times when setting vm configuration.
    pub set_vm_configuration: SharedIncMetric,
    /// Number of times when insertting vsock device.
    pub insert_vsock_device: SharedIncMetric,
    /// Number of times when insertting block device.
    pub insert_block_device: SharedIncMetric,
    /// Number of times when removing block device.
    pub remove_block_device: SharedIncMetric,
    /// Number of times when updating block device.
    pub update_block_device: SharedIncMetric,
    /// Number of times when insertting net device.
    pub insert_net_device: SharedIncMetric,
    /// Number of times when updating net device.
    pub update_net_interface: SharedIncMetric,
    /// Number of times when insertting fs device.
    pub insert_fs_device: SharedIncMetric,
    /// Number of times when manipulating fs backend.
    pub manipulate_fs_backend: SharedIncMetric,
    /// Number of times when updating fs device.
    pub update_fs_device: SharedIncMetric,
    /// Number of times when resizing vcpu.
    pub resize_vcpu: SharedIncMetric,
    /// Number of times when inserting memory device.
    pub insert_mem_device: SharedIncMetric,
    /// Number of times when inserting balloon device.
    pub insert_balloon_device: SharedIncMetric,
}

/// Structure storing all metrics while enforcing serialization support on them.
/// The type of the device metrics is HashMap<DeviceId, Arc<DeviceMetrics>> and the type of
/// non-device metrics is XXMetrics.
#[derive(Default, Serialize)]
pub struct DragonballMetrics {
    /// Metrics related to a vcpu's functioning.
    pub vcpu: HashMap<u32, Arc<VcpuMetrics>>,
    /// Metrics related to seccomp filtering.
    pub seccomp: SeccompMetrics,
    /// Metrics related to signals.
    pub signals: SignalMetrics,
    /// Metrics related to vmm action request.
    pub request: VmmRequestMetrics,
    /// Metrics related to i8032 device.
    #[cfg(target_arch = "x86_64")]
    pub i8042: Arc<I8042DeviceMetrics>,
    /// Metrics related to rtc device.
    #[cfg(target_arch = "aarch64")]
    pub rtc: Arc<RTCDeviceMetrics>,
    /// Metrics related to serial device.
    pub serial: HashMap<String, Arc<SerialDeviceMetrics>>,
    /// Metrics related to a MMIO transport.
    pub mmio: HashMap<String, Arc<MMIODeviceMetrics>>,
    #[cfg(feature = "virtio-vsock")]
    /// Metrics related to vsock devices.
    pub vsock: HashMap<String, Arc<VsockDeviceMetrics>>,
    #[cfg(feature = "virtio-net")]
    /// Metrics related to net device.
    pub net: HashMap<String, Arc<NetDeviceMetrics>>,
    #[cfg(feature = "virtio-blk")]
    /// Metrics related to block devices.
    pub block: HashMap<String, Arc<BlockDeviceMetrics>>,
    #[cfg(feature = "virtio-fs")]
    /// Metrics related to virtio-fs devices.
    pub fs: HashMap<String, Arc<FsDeviceMetrics>>,
    #[cfg(feature = "virtio-balloon")]
    /// Metrics related to balloon device.
    pub balloon: HashMap<String, Arc<BalloonDeviceMetrics>>,
    #[cfg(feature = "dbs-upcall")]
    /// Metrics related to upcall client.
    pub upcall: Arc<UpcallClientMetrics>,
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::thread;

    use dbs_utils::metric::IncMetric;

    use crate::metric::{VcpuMetrics, METRICS};

    #[test]
    fn test_read_map() {
        let metrics = Arc::new(VcpuMetrics::default());
        let vcpu_id: u32 = u32::MIN;
        METRICS
            .write()
            .unwrap()
            .vcpu
            .insert(vcpu_id, metrics.clone());
        metrics.failures.inc();
        assert_eq!(
            METRICS
                .read()
                .unwrap()
                .vcpu
                .get(&vcpu_id)
                .unwrap()
                .failures
                .count(),
            1
        );
    }

    #[test]
    fn test_metrics_count() {
        let metrics = Arc::new(VcpuMetrics::default());
        let vcpu_id: u32 = 65535;
        METRICS
            .write()
            .unwrap()
            .vcpu
            .insert(vcpu_id, metrics.clone());

        let metrics1 = metrics.clone();
        let thread1 = thread::spawn(move || {
            for _i in 0..10 {
                metrics1.exit_io_in.inc();
            }
        });

        let metrics2 = metrics.clone();
        let thread2 = thread::spawn(move || {
            for _i in 0..10 {
                metrics2.exit_io_in.inc();
            }
        });
        thread1.join().unwrap();
        thread2.join().unwrap();
        assert_eq!(
            METRICS
                .read()
                .unwrap()
                .vcpu
                .get(&vcpu_id)
                .unwrap()
                .exit_io_in
                .count(),
            20
        );
    }

    #[test]
    fn test_rw_lock() {
        let metrics = Arc::new(VcpuMetrics::default());
        let vcpu_id: u32 = u32::MAX;
        METRICS
            .write()
            .unwrap()
            .vcpu
            .insert(vcpu_id, metrics.clone());

        let write_thread = thread::spawn(move || {
            for _ in 0..10 {
                let metrics = Arc::new(VcpuMetrics::default());
                let vcpu_id: u32 = 128;
                METRICS
                    .write()
                    .unwrap()
                    .vcpu
                    .insert(vcpu_id, metrics.clone());
            }
        });

        let read_thread = thread::spawn(move || {
            for _ in 0..10 {
                assert_eq!(
                    METRICS
                        .read()
                        .unwrap()
                        .vcpu
                        .get(&vcpu_id)
                        .unwrap()
                        .failures
                        .count(),
                    0
                );
            }
        });
        write_thread.join().unwrap();
        read_thread.join().unwrap();
    }
}
