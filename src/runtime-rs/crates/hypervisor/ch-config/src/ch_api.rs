// Copyright (c) 2022-2023 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0

use crate::net_util::MAC_ADDR_LEN;
use crate::{
    ConsoleConfig, ConsoleOutputMode, CpuTopology, CpusConfig, DeviceConfig, FsConfig, MacAddr,
    MemoryConfig, NetConfig, PayloadConfig, PmemConfig, RngConfig, VmConfig, VsockConfig,
};
use anyhow::{anyhow, Context, Result};
use api_client::simple_api_full_command_and_response;

use std::fmt::Display;
use std::net::Ipv4Addr;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use tokio::task;

pub async fn cloud_hypervisor_vmm_ping(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(&mut socket, "GET", "vmm.ping", None)
            .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vmm_shutdown(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response =
            simple_api_full_command_and_response(&mut socket, "PUT", "vmm.shutdown", None)
                .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_create(
    sandbox_path: String,
    vsock_socket_path: String,
    mut socket: UnixStream,
    shared_fs_devices: Option<Vec<FsConfig>>,
    pmem_devices: Option<Vec<PmemConfig>>,
) -> Result<Option<String>> {
    let cfg = cloud_hypervisor_vm_create_cfg(
        sandbox_path,
        vsock_socket_path,
        shared_fs_devices,
        pmem_devices,
    )
    .await?;

    let serialised = serde_json::to_string_pretty(&cfg)?;

    task::spawn_blocking(move || -> Result<Option<String>> {
        let data = Some(serialised.as_str());

        let response = simple_api_full_command_and_response(&mut socket, "PUT", "vm.create", data)
            .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_start(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(&mut socket, "PUT", "vm.boot", None)
            .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_stop(mut socket: UnixStream) -> Result<Option<String>> {
    task::spawn_blocking(move || -> Result<Option<String>> {
        let response =
            simple_api_full_command_and_response(&mut socket, "PUT", "vm.shutdown", None)
                .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

#[allow(dead_code)]
pub async fn cloud_hypervisor_vm_device_add(mut socket: UnixStream) -> Result<Option<String>> {
    let device_config = DeviceConfig::default();

    task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-device",
            Some(&serde_json::to_string(&device_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?
}

pub async fn cloud_hypervisor_vm_fs_add(
    mut socket: UnixStream,
    fs_config: FsConfig,
) -> Result<Option<String>> {
    let result = task::spawn_blocking(move || -> Result<Option<String>> {
        let response = simple_api_full_command_and_response(
            &mut socket,
            "PUT",
            "vm.add-fs",
            Some(&serde_json::to_string(&fs_config)?),
        )
        .map_err(|e| anyhow!(e))?;

        Ok(response)
    })
    .await?;

    result
}

pub async fn cloud_hypervisor_vm_create_cfg(
    // FIXME:
    _sandbox_path: String,
    vsock_socket_path: String,
    shared_fs_devices: Option<Vec<FsConfig>>,
    pmem_devices: Option<Vec<PmemConfig>>,
) -> Result<VmConfig> {
    let topology = CpuTopology {
        threads_per_core: 1,
        cores_per_die: 12,
        dies_per_package: 1,
        packages: 1,
    };

    let cpus = CpusConfig {
        boot_vcpus: 1,
        max_vcpus: 12,
        max_phys_bits: 46,
        topology: Some(topology),
        ..Default::default()
    };

    let rng = RngConfig {
        src: PathBuf::from("/dev/urandom"),
        ..Default::default()
    };

    let kernel_args = vec![
        "root=/dev/pmem0p1",
        "rootflags=dax,data=ordered,errors=remount-ro",
        "ro",
        "rootfstype=ext4",
        "panic=1",
        "no_timer_check",
        "noreplace-smp",
        "console=ttyS0,115200n8",
        "systemd.log_target=console",
        "systemd.unit=kata-containers",
        "systemd.mask=systemd-networkd.service",
        "systemd.mask=systemd-networkd.socket",
        "agent.log=debug",
    ];

    let cmdline = kernel_args.join(" ");

    let kernel = PathBuf::from("/opt/kata/share/kata-containers/vmlinux.container");

    // Note that PmemConfig replaces the PayloadConfig.initrd.
    let payload = PayloadConfig {
        kernel: Some(kernel),
        cmdline: Some(cmdline),
        ..Default::default()
    };

    let serial = ConsoleConfig {
        mode: ConsoleOutputMode::Tty,
        ..Default::default()
    };

    let ip = Ipv4Addr::new(192, 168, 10, 10);
    let mask = Ipv4Addr::new(255, 255, 255, 0);

    let mac_str = "12:34:56:78:90:01";

    let mac = parse_mac(mac_str)?;

    let network = NetConfig {
        ip,
        mask,
        mac,
        ..Default::default()
    };

    let memory = MemoryConfig {
        size: (1024 * 1024 * 2048),

        // Required
        shared: true,

        prefault: false,
        hugepages: false,
        mergeable: false,

        // FIXME:
        hotplug_size: Some(16475226112),

        ..Default::default()
    };

    let fs = shared_fs_devices;
    let pmem = pmem_devices;

    let vsock = VsockConfig {
        cid: 3,
        socket: PathBuf::from(vsock_socket_path),
        ..Default::default()
    };

    let cfg = VmConfig {
        cpus,
        memory,
        fs,
        serial,
        pmem,
        payload: Some(payload),
        vsock: Some(vsock),
        rng,
        net: Some(vec![network]),
        ..Default::default()
    };

    Ok(cfg)
}

fn parse_mac<S>(s: &S) -> Result<MacAddr>
where
    S: AsRef<str> + ?Sized + Display,
{
    let v: Vec<&str> = s.as_ref().split(':').collect();
    let mut bytes = [0u8; MAC_ADDR_LEN];

    if v.len() != MAC_ADDR_LEN {
        return Err(anyhow!(
            "invalid MAC {} (length {}, expected {})",
            s,
            v.len(),
            MAC_ADDR_LEN
        ));
    }

    for i in 0..MAC_ADDR_LEN {
        if v[i].len() != 2 {
            return Err(anyhow!(
                "invalid MAC {} (segment {} length {}, expected {})",
                s,
                i,
                v.len(),
                2
            ));
        }

        bytes[i] =
            u8::from_str_radix(v[i], 16).context(format!("failed to parse MAC address: {}", s))?;
    }

    Ok(MacAddr { bytes })
}
