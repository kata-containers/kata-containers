// Copyright (c) 2019 Ant Financial
// Copyright (c) 2024 Intel Corporation
//
// SPDX-License-Identifier: Apache-2.0
//

use self::block_device_handler::{VirtioBlkMmioDeviceHandler, VirtioBlkPciDeviceHandler};
use self::nvdimm_device_handler::VirtioNvdimmDeviceHandler;
use self::scsi_device_handler::ScsiDeviceHandler;
use self::vfio_device_handler::{VfioApDeviceHandler, VfioPciDeviceHandler};
use crate::pci;
use crate::sandbox::PciHostGuestMapping;
use crate::sandbox::Sandbox;
use anyhow::{anyhow, Context, Result};
use cdi::annotations::parse_annotations;
use cdi::cache::{new_cache, with_auto_refresh, CdiOption};
use cdi::spec_dirs::with_spec_dirs;
use container_device_interface as cdi;
use kata_sys_util::pcilibs::{is_vfio_device_type, snapshot_infiniband};
use kata_types::device::DeviceHandlerManager;
use nix::sys::stat;
use oci::{LinuxDeviceCgroup, Spec};
use oci_spec::runtime as oci;
use protocols::agent::Device;
use slog::Logger;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::os::unix::prelude::FileTypeExt;
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time;
use tokio::time::Duration;
use tracing::instrument;

pub mod block_device_handler;
pub mod network_device_handler;
pub mod nvdimm_device_handler;
pub mod scsi_device_handler;
pub mod vfio_device_handler;

pub const BLOCK: &str = "block";

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    // Device type, "b" for block device and "c" for character device
    cgroup_type: String,
    // The major and minor numbers for the device within the guest
    guest_major: i64,
    guest_minor: i64,
}

impl DeviceInfo {
    /// Create a device info.
    ///
    /// # Arguments
    ///
    /// * `vm_path` - Device's vm path.
    /// * `is_rdev` - If the vm_path is a device, set to true. If the
    ///   vm_path is a file in a device, set to false.
    pub fn new(vm_path: &str, is_rdev: bool) -> Result<Self> {
        let cgroup_type;
        let devid;

        let vm_path = PathBuf::from(vm_path);
        if !vm_path.exists() {
            return Err(anyhow!("VM device path {:?} doesn't exist", vm_path));
        }

        let metadata = fs::metadata(&vm_path)?;

        if is_rdev {
            devid = metadata.rdev();
            let file_type = metadata.file_type();
            if file_type.is_block_device() {
                cgroup_type = String::from("b");
            } else if file_type.is_char_device() {
                cgroup_type = String::from("c");
            } else {
                return Err(anyhow!("Unknown device {:?}'s cgroup type", vm_path));
            }
        } else {
            devid = metadata.dev();
            cgroup_type = String::from("b");
        }

        let guest_major = stat::major(devid) as i64;
        let guest_minor = stat::minor(devid) as i64;

        Ok(DeviceInfo {
            cgroup_type,
            guest_major,
            guest_minor,
        })
    }
}

// Represents the device-node and resource related updates to the OCI
// spec needed for a particular device
#[derive(Debug, Clone)]
struct DevUpdate {
    info: DeviceInfo,
    // an optional new path to update the device to in the "inner" container
    // specification
    final_path: Option<String>,
}

impl DevUpdate {
    fn new(vm_path: &str, final_path: &str) -> Result<Self> {
        Ok(DevUpdate {
            final_path: Some(final_path.to_owned()),
            ..DeviceInfo::new(vm_path, true)?.into()
        })
    }
}

impl From<DeviceInfo> for DevUpdate {
    fn from(info: DeviceInfo) -> Self {
        DevUpdate {
            info,
            final_path: None,
        }
    }
}

// Represents the updates to the OCI spec needed for a particular device
#[derive(Debug, Clone, Default)]
pub struct SpecUpdate {
    dev: Option<DevUpdate>,
    // optional corrections for PCI addresses
    pci: Vec<(pci::Address, pci::Address)>,
}

impl<T: Into<DevUpdate>> From<T> for SpecUpdate {
    fn from(dev: T) -> Self {
        SpecUpdate {
            dev: Some(dev.into()),
            pci: Vec::new(),
        }
    }
}

#[derive(Debug)]
pub struct DeviceContext<'a> {
    logger: &'a Logger,
    sandbox: &'a Arc<Mutex<Sandbox>>,
}

/// Trait object to handle device.
#[async_trait::async_trait]
pub trait DeviceHandler: Send + Sync {
    /// Handle the device
    async fn device_handler(&self, device: &Device, ctx: &mut DeviceContext) -> Result<SpecUpdate>;

    /// Return the driver types that the handler manages.
    fn driver_types(&self) -> &[&str];
}

#[rustfmt::skip]
lazy_static! {
    pub static ref DEVICE_HANDLERS: DeviceHandlerManager<Arc<dyn DeviceHandler>> = {
        let mut manager: DeviceHandlerManager<Arc<dyn DeviceHandler>> = DeviceHandlerManager::new();

        let handlers: Vec<Arc<dyn DeviceHandler>> = vec![
            Arc::new(VirtioBlkMmioDeviceHandler {}),
            Arc::new(VirtioBlkPciDeviceHandler {}),
            Arc::new(VirtioNvdimmDeviceHandler {}),
            Arc::new(ScsiDeviceHandler {}),
            Arc::new(VfioPciDeviceHandler {}),
            Arc::new(VfioApDeviceHandler {}),
            #[cfg(target_arch = "s390x")]
            Arc::new(self::block_device_handler::VirtioBlkCcwDeviceHandler {}),
        ];

        for handler in handlers {
            manager.add_handler(handler.driver_types(), handler.clone()).unwrap();
        }
        manager
    };
}

#[instrument]
pub async fn add_devices(
    cid: &String,
    logger: &Logger,
    devices: &[Device],
    spec: &mut Spec,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    let mut dev_updates = HashMap::<&str, DevUpdate>::with_capacity(devices.len());

    for device in devices.iter() {
        validate_device(logger, device, sandbox).await?;
        if let Some(handler) = DEVICE_HANDLERS.handler(&device.type_) {
            let mut ctx = DeviceContext { logger, sandbox };

            match handler.device_handler(device, &mut ctx).await {
                Ok(update) => {
                    if let Some(dev_update) = update.dev {
                        if dev_updates
                            .insert(&device.container_path, dev_update.clone())
                            .is_some()
                        {
                            return Err(anyhow!(
                                "Conflicting device updates for {}",
                                &device.container_path
                            ));
                        }

                        // Update cgroup to allow all devices added to guest.
                        insert_devices_cgroup_rule(logger, spec, &dev_update.info, true, "rwm")
                            .context("Update device cgroup")?;
                    }

                    let mut sb = sandbox.lock().await;
                    let mut host_guest: PciHostGuestMapping = HashMap::new();
                    for (host, guest) in update.pci {
                        if let Some(other_guest) = host_guest.insert(host, guest) {
                            return Err(anyhow!(
                                "Conflicting guest address for host device {} ({} versus {})",
                                host,
                                guest,
                                other_guest
                            ));
                        }
                    }
                    // Save all the host -> guest mappings per container upon
                    // removal of the container, the mappings will be removed
                    sb.pcimap.insert(cid.clone(), host_guest);
                }
                Err(e) => {
                    error!(logger, "failed to add devices, error: {e:?}");
                    return Err(e);
                }
            }
        } else {
            return Err(anyhow!(
                "Failed to find the device handler {}",
                device.type_
            ));
        }
    }

    if let Some(process) = spec.process_mut() {
        let env_vec: &mut Vec<String> =
            &mut process.env_mut().get_or_insert_with(Vec::new).to_vec();
        update_env_pci(cid, env_vec, &sandbox.lock().await.pcimap)?
    }

    // Expose any RDMA / InfiniBand char devices the guest kernel
    // created for cold-plugged VFs, but only for containers that
    // actually requested at least one VFIO device. Without this gate
    // every container — including unrelated workloads sharing the
    // sandbox — would get `/dev/infiniband/*` mapped in plus matching
    // device cgroup `allow` entries, which weakens per-container
    // isolation for no benefit. Cold-plug VFIO + guest-kernel RDMA
    // is the only flow that needs `uverbs<N>` / `rdma_cm`, and that
    // flow always shows up in `devices` with one of the VFIO driver
    // types.
    if container_has_vfio_device(devices) {
        expose_guest_infiniband_devices(logger, spec).context("expose_guest_infiniband_devices")?;
    }

    update_spec_devices(logger, spec, dev_updates)
}

/// Returns true if `devices` contains at least one entry whose
/// `type_` matches one of the VFIO driver constants. Callers use
/// this to gate host-wide VFIO-only side effects (e.g. exposing
/// `/dev/infiniband/*` to the container) so the agent does not
/// widen device access for unrelated containers.
fn container_has_vfio_device(devices: &[Device]) -> bool {
    devices
        .iter()
        .any(|d| is_vfio_device_type(d.type_.as_str()))
}

pub fn dump_nvidia_cdi_yaml(logger: &Logger) -> Result<()> {
    let file_path = "/var/run/cdi/nvidia.yaml";
    let path = PathBuf::from(file_path);

    if !path.exists() {
        debug!(
            logger,
            "CDI spec file does not exist, skipping: {}", file_path
        );
        return Ok(());
    }

    let metadata = fs::metadata(&path)?;
    debug!(
        logger,
        "CDI spec at {}: {} bytes",
        file_path,
        metadata.len()
    );

    Ok(())
}

const VISIBLE_CDI_DEVICES_ENV: &str = "VISIBLE_CDI_DEVICES";

/// Translate a container's VISIBLE_CDI_DEVICES environment variable into a
/// list of fully-qualified CDI device names (e.g. "nvidia.com/gpu=all" or
/// "nvidia.com/ib=0"). The variable is a ':'-separated list of
/// "<cdi-kind>=<devices>" entries.
/// Returns an empty vector when the variable is unset, empty, or set to one of
/// the sentinel values "none"/"void" (following the NVIDIA *_VISIBLE_DEVICES
/// convention) that explicitly request no CDI devices.
pub fn cdi_devices_from_visible_devices(spec: &Spec) -> Result<Vec<String>> {
    let prefix = format!("{VISIBLE_CDI_DEVICES_ENV}=");
    let value = spec
        .process()
        .as_ref()
        .and_then(|p| p.env().as_ref())
        .and_then(|env| env.iter().find_map(|e| e.strip_prefix(prefix.as_str())));

    let value = match value {
        Some(v) => v.trim(),
        None => return Ok(Vec::new()),
    };

    match value {
        "" | "none" | "void" => Ok(Vec::new()),
        list => {
            let mut devices = Vec::new();
            for entry in list.split(':').map(str::trim) {
                devices.extend(cdi_devices_from_entry(entry)?);
            }
            Ok(devices)
        }
    }
}

// Translate a single "<cdi-kind>=<devices>" entry (e.g. "nvidia.com/ib=0,1")
// into a list of fully-qualified CDI device names (e.g.
// ["nvidia.com/ib=0", "nvidia.com/ib=1"]). <devices> is a comma-separated list
// of "all" or non-negative device indices. The CDI kind must be specified
// explicitly, there is no default.
fn cdi_devices_from_entry(entry: &str) -> Result<Vec<String>> {
    let (kind, devices) = entry.split_once('=').ok_or_else(|| {
        anyhow!(
            "invalid {}: entry {:?} is missing a CDI kind (expected \"<kind>=<devices>\")",
            VISIBLE_CDI_DEVICES_ENV,
            entry
        )
    })?;

    let kind = kind.trim();
    if kind.is_empty() {
        return Err(anyhow!(
            "invalid {}: entry {:?} has an empty CDI kind",
            VISIBLE_CDI_DEVICES_ENV,
            entry
        ));
    }

    let devices = devices.trim();
    if devices.is_empty() {
        return Err(anyhow!(
            "invalid {}: CDI kind {:?} has no devices",
            VISIBLE_CDI_DEVICES_ENV,
            kind
        ));
    }

    let tokens: Vec<&str> = devices.split(',').map(str::trim).collect();

    if tokens.len() > 1 && tokens.contains(&"all") {
        return Err(anyhow!(
            "invalid {}: CDI kind {:?} mixes \"all\" with explicit device indices",
            VISIBLE_CDI_DEVICES_ENV,
            kind
        ));
    }

    tokens
        .into_iter()
        .map(|token| token_to_kind_and_device(kind, token))
        .collect()
}

// Translate a single device token into a fully-qualified CDI device name. The token must be either
// "all" or a non-negative integer device index.
fn token_to_kind_and_device(kind: &str, token: &str) -> Result<String> {
    if token == "all" {
        return Ok(format!("{kind}=all"));
    }

    token
        .parse::<u32>()
        .map(|n| format!("{kind}={n}"))
        .map_err(|_| {
            anyhow!(
                "invalid {}: {:?} is not \"all\" or a non-negative device index for CDI kind {:?}",
                VISIBLE_CDI_DEVICES_ENV,
                token,
                kind
            )
        })
}

fn cdi_kind(device: &str) -> Option<&str> {
    device.split_once('=').map(|(kind, _)| kind)
}

/// Return the requested devices that can never be injected: those whose CDI
/// kind is already present in `available` but which name a device the kind does
/// not provide (e.g. "nvidia.com/gpu=5" when only GPUs 0-3 exist).
///
/// A requested device whose kind is entirely absent from `available` is
/// deliberately omitted: its CDI spec may simply not have been generated yet,
/// and the caller should keep waiting for it rather than fail.
fn unsatisfiable_cdi_devices(available: &[String], requested: &[String]) -> Vec<String> {
    let known_kinds: HashSet<&str> = available.iter().filter_map(|d| cdi_kind(d)).collect();
    let available: HashSet<&str> = available.iter().map(String::as_str).collect();

    requested
        .iter()
        .filter(|d| match cdi_kind(d) {
            Some(kind) => known_kinds.contains(kind) && !available.contains(d.as_str()),
            None => false,
        })
        .cloned()
        .collect()
}

#[instrument]
pub async fn handle_cdi_devices(
    logger: &Logger,
    spec: &mut Spec,
    spec_dir: &str,
    cdi_timeout: time::Duration,
    extra_devices: &[String],
) -> Result<()> {
    if let Some(container_type) = spec
        .annotations()
        .as_ref()
        .and_then(|a| a.get("io.katacontainers.pkg.oci.container_type"))
    {
        if container_type == "pod_sandbox" {
            return Ok(());
        }
    }

    let mut devices = match spec.annotations().as_ref() {
        Some(annotations) => parse_annotations(annotations)?.1,
        None => Vec::new(),
    };

    // Devices requested via the container's VISIBLE_CDI_DEVICES environment
    // variable are merged with any devices the host injected through
    // cdi.k8s.io/* annotations.
    for dev in extra_devices {
        if !devices.contains(dev) {
            devices.push(dev.clone());
        }
    }

    if devices.is_empty() {
        info!(logger, "no CDI annotations, no devices to inject");
        return Ok(());
    }
    // Explicitly set the cache options to disable auto-refresh and
    // to use the single spec dir "/var/run/cdi" for tests it can be overridden
    let options: Vec<CdiOption> = vec![with_auto_refresh(false), with_spec_dirs(&[spec_dir])];
    let cache: Arc<std::sync::Mutex<cdi::cache::Cache>> = new_cache(options);

    for i in 0..=cdi_timeout.as_secs() {
        let (unsatisfiable, inject_result) = {
            // Lock cache within this scope, std::sync::Mutex has no Send
            // and await will not work with time::sleep
            let mut cache = cache.lock().unwrap();
            match cache.refresh() {
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow!("error refreshing cache: {:?}", e));
                }
            }
            let unsatisfiable = unsatisfiable_cdi_devices(&cache.list_devices(), &devices);
            let inject_result = cache.inject_devices(Some(spec), devices.clone());
            (unsatisfiable, inject_result)
        };

        match inject_result {
            Ok(_) => {
                info!(
                    logger,
                    "all devices injected successfully, modified CDI container spec: {:?}", &spec
                );
                return Ok(());
            }
            Err(e) => {
                if !unsatisfiable.is_empty() {
                    return Err(anyhow!(
                        "CDI device(s) {:?} do not exist; their CDI kind(s) are present \
                         in {} but do not provide them: {:?}",
                        unsatisfiable,
                        spec_dir,
                        e
                    ));
                }
                info!(
                    logger,
                    "waiting for CDI spec(s) to be generated ({} of {} max tries) {:?}",
                    i,
                    cdi_timeout.as_secs(),
                    e
                );
                time::sleep(Duration::from_secs(1)).await;
            }
        }
    }
    Err(anyhow!(
        "failed to inject devices after CDI timeout of {} seconds",
        cdi_timeout.as_secs()
    ))
}

#[instrument]
async fn validate_device(
    logger: &Logger,
    device: &Device,
    sandbox: &Arc<Mutex<Sandbox>>,
) -> Result<()> {
    // log before validation to help with debugging gRPC protocol version differences.
    info!(
        logger,
        "device-id: {}, device-type: {}, device-vm-path: {}, device-container-path: {}, device-options: {:?}",
        device.id, device.type_, device.vm_path, device.container_path, device.options
    );

    if device.type_.is_empty() {
        return Err(anyhow!("invalid type for device {:?}", device));
    }

    if device.id.is_empty() && device.vm_path.is_empty() {
        return Err(anyhow!("invalid ID and VM path for device {:?}", device));
    }

    if device.container_path.is_empty() {
        return Err(anyhow!("invalid container path for device {:?}", device));
    }
    return Ok(());
}

// Insert a devices cgroup rule to control access to device.
#[instrument]
pub fn insert_devices_cgroup_rule(
    logger: &Logger,
    spec: &mut Spec,
    dev_info: &DeviceInfo,
    allow: bool,
    access: &str,
) -> Result<()> {
    let linux = spec
        .linux_mut()
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't container linux field"))?;
    let devcgrp_type = dev_info
        .cgroup_type
        .parse::<oci::LinuxDeviceType>()
        .context(format!(
            "Failed to parse {:?} to Enum LinuxDeviceType",
            dev_info.cgroup_type
        ))?;
    let linux_resource = &mut oci::LinuxResources::default();
    let resource = linux.resources_mut().as_mut().unwrap_or(linux_resource);
    let mut device_cgrp = LinuxDeviceCgroup::default();
    device_cgrp.set_allow(allow);
    device_cgrp.set_major(Some(dev_info.guest_major));
    device_cgrp.set_minor(Some(dev_info.guest_minor));
    device_cgrp.set_typ(Some(devcgrp_type));
    device_cgrp.set_access(Some(access.to_owned()));

    debug!(
        logger,
        "Insert a devices cgroup rule";
        "linux_device_cgroup" => device_cgrp.allow(),
        "guest_major" => device_cgrp.major(),
        "guest_minor" => device_cgrp.minor(),
        "type" => device_cgrp.typ().unwrap().as_str(),
        "access" => device_cgrp.access().as_ref().unwrap().as_str(),
    );

    if let Some(devices) = resource.devices_mut() {
        devices.push(device_cgrp);
    } else {
        resource.set_devices(Some(vec![device_cgrp]));
    }

    Ok(())
}

// update_env_pci alters PCI addresses in a set of environment
// variables to be correct for the VM instead of the host.  It is
// given a map of (host address => guest address)
#[instrument]
pub fn update_env_pci(
    cid: &String,
    env: &mut [String],
    pcimap: &HashMap<String, PciHostGuestMapping>,
) -> Result<()> {
    // SR-IOV device plugin may add two environment variables for one resource:
    // - PCIDEVICE_<prefix>_<resource-name>: a list of PCI device ids separated by comma
    // - PCIDEVICE_<prefix>_<resource-name>_INFO: detailed info in JSON for above PCI devices
    // Both environment variables hold information about the same set of PCI devices.
    // Below code updates both of them in two passes:
    // - 1st pass updates PCIDEVICE_<prefix>_<resource-name> and collects host to guest PCI address mapping
    let mut pci_dev_map: HashMap<String, HashMap<String, String>> = HashMap::new();
    'env_loop: for envvar in env.iter_mut() {
        let eqpos = envvar
            .find('=')
            .ok_or_else(|| anyhow!("Malformed OCI env entry {:?}", envvar))?;

        let (name, eqval) = envvar.split_at(eqpos);
        let val = &eqval[1..];

        if !name.starts_with("PCIDEVICE_") || name.ends_with("_INFO") {
            continue;
        }

        let mut addr_map: HashMap<String, String> = HashMap::new();
        let mut guest_addrs = Vec::<String>::new();
        let mut translation_skipped = false;
        for host_addr_str in val.split(',') {
            let host_addr = match pci::Address::from_str(host_addr_str) {
                Ok(addr) => addr,
                Err(_) => {
                    tracing::info!(
                        name,
                        host_addr_str,
                        "skipping non-PCI address in PCIDEVICE env var"
                    );
                    continue 'env_loop;
                }
            };
            let host_guest = match pcimap.get(cid) {
                Some(m) => m,
                None => {
                    tracing::warn!(
                        cid = cid.as_str(),
                        env_name = name,
                        host_addr = host_addr_str,
                        "update_env_pci: no per-container pcimap; leaving PCIDEVICE env var untranslated"
                    );
                    translation_skipped = true;
                    break;
                }
            };
            let guest_addr = match host_guest.get(&host_addr) {
                Some(g) => g,
                None => {
                    tracing::warn!(
                        cid = cid.as_str(),
                        env_name = name,
                        host_addr = host_addr_str,
                        pcimap_size = host_guest.len(),
                        "update_env_pci: host PCI address missing in pcimap; leaving PCIDEVICE env var untranslated"
                    );
                    translation_skipped = true;
                    break;
                }
            };

            guest_addrs.push(format!("{guest_addr}"));
            addr_map.insert(host_addr_str.to_string(), format!("{guest_addr}"));
        }

        if translation_skipped {
            // Keep both PCIDEVICE_* and matching *_INFO untouched.
            continue 'env_loop;
        }

        pci_dev_map.insert(format!("{name}_INFO"), addr_map);

        envvar.replace_range(eqpos + 1.., guest_addrs.join(",").as_str());
    }

    // - 2nd pass update PCIDEVICE_<prefix>_<resource-name>_INFO if it exists
    for envvar in env.iter_mut() {
        let eqpos = envvar
            .find('=')
            .ok_or_else(|| anyhow!("Malformed OCI env entry {:?}", envvar))?;

        let (name, _) = envvar.split_at(eqpos);
        if !(name.starts_with("PCIDEVICE_") && name.ends_with("_INFO")) {
            continue;
        }

        if let Some(addr_map) = pci_dev_map.get(name) {
            for (host_addr, guest_addr) in addr_map {
                *envvar = envvar.replace(host_addr, guest_addr);
            }
        }
    }

    Ok(())
}

// update_spec_devices updates the device list in the OCI spec to make
// it include details appropriate for the VM, instead of the host.  It
// is given a map of (container_path => update) where:
//     container_path: the path to the device in the original OCI spec
//     update: information on changes to make to the device
#[instrument]
fn update_spec_devices(
    logger: &Logger,
    spec: &mut Spec,
    mut updates: HashMap<&str, DevUpdate>,
) -> Result<()> {
    let linux = spec
        .linux_mut()
        .as_mut()
        .ok_or_else(|| anyhow!("Spec didn't contain linux field"))?;
    let mut res_updates = HashMap::<(String, i64, i64), DeviceInfo>::with_capacity(updates.len());

    let mut default_devices = Vec::new();
    let linux_devices = linux.devices_mut().as_mut().unwrap_or(&mut default_devices);
    for specdev in linux_devices.iter_mut() {
        let devtype = specdev.typ().as_str().to_string();
        if let Some(update) = updates.remove(specdev.path().clone().display().to_string().as_str())
        {
            let host_major = specdev.major();
            let host_minor = specdev.minor();

            info!(
                logger,
                "update_spec_devices() updating device";
                "container_path" => &specdev.path().display().to_string(),
                "type" => &devtype,
                "host_major" => host_major,
                "host_minor" => host_minor,
                "guest_major" => update.info.guest_major,
                "guest_minor" => update.info.guest_minor,
                "final_path" => update.final_path.as_ref(),
            );

            specdev.set_major(update.info.guest_major);
            specdev.set_minor(update.info.guest_minor);
            if let Some(final_path) = update.final_path {
                specdev.set_path(PathBuf::from(&final_path));
            }

            if res_updates
                .insert((devtype, host_major, host_minor), update.info)
                .is_some()
            {
                return Err(anyhow!(
                    "Conflicting resource updates for host_major={} host_minor={}",
                    host_major,
                    host_minor
                ));
            }
        }
    }

    // Make sure we applied all of our updates
    if !updates.is_empty() {
        return Err(anyhow!(
            "Missing devices in OCI spec: {:?}",
            updates
                .keys()
                .map(|d| format!("{d:?}"))
                .collect::<Vec<_>>()
                .join(" ")
        ));
    }

    if let Some(resources) = linux.resources_mut().as_mut() {
        if let Some(resources_devices) = resources.devices_mut().as_mut() {
            for d in resources_devices.iter_mut() {
                let dev_type = d.typ().unwrap_or_default().as_str().to_string();
                if let (Some(host_major), Some(host_minor)) = (d.major(), d.minor()) {
                    if let Some(update) =
                        res_updates.get(&(dev_type.clone(), host_major, host_minor))
                    {
                        info!(
                            logger,
                            "update_spec_devices() updating resource";
                            "type" => &dev_type,
                            "host_major" => host_major,
                            "host_minor" => host_minor,
                            "guest_major" => update.guest_major,
                            "guest_minor" => update.guest_minor,
                        );

                        d.set_major(Some(update.guest_major));
                        d.set_minor(Some(update.guest_minor));
                    }
                }
            }
        }
    }

    Ok(())
}

fn parse_pci_bdf_name(name: &str) -> Option<pci::Address> {
    pci::Address::from_str(name).ok()
}

fn bus_of_addr(addr: &pci::Address) -> Result<String> {
    // addr.to_string() format: "0000:01:00.0"
    let s = addr.to_string();
    let mut parts = s.split(':');

    let domain = parts
        .next()
        .ok_or_else(|| anyhow!("bad pci address {}", s))?;
    let bus = parts
        .next()
        .ok_or_else(|| anyhow!("bad pci address {}", s))?;

    Ok(format!("{domain}:{bus}"))
}

fn unique_bus_from_pci_addresses(addrs: &[pci::Address]) -> Result<String> {
    let mut buses = addrs.iter().map(bus_of_addr).collect::<Result<Vec<_>>>()?;

    buses.sort();
    buses.dedup();

    match buses.len() {
        1 => Ok(buses[0].clone()),
        0 => Err(anyhow!("no downstream PCI devices found")),
        _ => Err(anyhow!("multiple downstream buses found: {:?}", buses)),
    }
}

fn read_single_bus_from_pci_bus_dir(bridgebuspath: &PathBuf) -> Result<String> {
    let mut files = Vec::new();

    for entry in fs::read_dir(bridgebuspath)? {
        files.push(entry?);
    }

    if files.len() != 1 {
        return Err(anyhow!(
            "expected exactly one PCI bus in {:?}, got {}",
            bridgebuspath,
            files.len()
        ));
    }

    files[0]
        .file_name()
        .into_string()
        .map_err(|e| anyhow!("bad filename under {:?}: {:?}", bridgebuspath, e))
}

fn infer_bus_from_child_devices(devpath: &PathBuf) -> Result<String> {
    let mut child_pci_addrs = Vec::new();

    for entry in fs::read_dir(devpath)? {
        let entry = entry?;
        let file_type = entry.file_type()?;

        if !file_type.is_dir() {
            continue;
        }

        let name = entry.file_name();
        let name = name
            .to_str()
            .ok_or_else(|| anyhow!("non-utf8 filename under {:?}: {:?}", devpath, name))?;

        if let Some(addr) = parse_pci_bdf_name(name) {
            child_pci_addrs.push(addr);
        }
    }

    unique_bus_from_pci_addresses(&child_pci_addrs).with_context(|| {
        format!(
            "failed to infer downstream bus from child PCI devices under {:?}",
            devpath
        )
    })
}

fn get_next_bus_from_bridge(devpath: &PathBuf) -> Result<String> {
    let bridgebuspath = devpath.join("pci_bus");

    if bridgebuspath.exists() {
        return read_single_bus_from_pci_bus_dir(&bridgebuspath)
            .with_context(|| format!("failed to read downstream bus from {:?}", bridgebuspath));
    }

    infer_bus_from_child_devices(devpath).with_context(|| {
        format!(
            "bridge {:?} has no pci_bus directory; fallback to child device scan failed",
            devpath
        )
    })
}

// pcipath_to_sysfs fetches the sysfs path for a PCI path, relative to
// the sysfs path for the PCI host bridge, based on the PCI path
// provided.
#[instrument]
pub fn pcipath_to_sysfs(root_bus_sysfs: &str, pcipath: &pci::Path) -> Result<String> {
    let mut bus = "0000:00".to_string();
    let mut relpath = String::new();

    if pcipath.is_empty() {
        return Err(anyhow!("empty PCI path"));
    }

    for i in 0..pcipath.len() {
        let bdf = format!("{}:{}", bus, pcipath[i]);

        relpath = format!("{relpath}/{bdf}");

        if i == pcipath.len() - 1 {
            // Final device need not be a bridge
            break;
        }

        let devpath = PathBuf::from(root_bus_sysfs).join(relpath.trim_start_matches('/'));

        bus = get_next_bus_from_bridge(&devpath).with_context(|| {
            format!(
                "failed to resolve next bus for PCI path element {} (device {}) under root {}",
                i, bdf, root_bus_sysfs
            )
        })?;
    }

    Ok(relpath)
}

#[instrument]
pub fn online_device(path: &str) -> Result<()> {
    // For virtio-mem-ccw (s390x), hotplugged memory blocks must land in the
    // MOVABLE zone so they can be offlined later during hot-unplug.  Writing
    // "1" (equivalent to "online") places blocks in NORMAL, which the kernel
    // refuses to offline.  Check valid_zones first; fall back to "1" when the
    // file is absent or when only the Normal zone is advertised, preserving
    // existing behaviour on all other architectures and device types.
    let valid_zones_path = std::path::Path::new(path)
        .parent()
        .map(|p| p.join("valid_zones"));
    let value = valid_zones_path
        .and_then(|p| fs::read_to_string(p).ok())
        .map(|z| {
            if z.contains("Movable") {
                "online_movable"
            } else {
                "1"
            }
        })
        .unwrap_or("1");
    fs::write(path, value)?;
    Ok(())
}

/// Walk `/dev/infiniband/` and append every char device found to the
/// workload container's OCI spec, so that applications inside the
/// container can use the guest's RDMA stack.
///
/// Only called when the container has at least one VFIO device
/// (`container_has_vfio_device`), so this is a no-op for unrelated
/// containers sharing the sandbox.
///
/// If `/dev/infiniband/` does not exist or is empty the function
/// returns `Ok(())` immediately — the guest simply does not have IB
/// devices (no mlx5_ib or VF not yet rebound).
fn expose_guest_infiniband_devices(logger: &Logger, spec: &mut Spec) -> Result<()> {
    let ib_dir = std::path::Path::new("/dev/infiniband");
    if !ib_dir.exists() {
        info!(
            logger,
            "expose_guest_infiniband_devices: /dev/infiniband does not \
             exist, skipping (no IB driver in guest, or VF not yet rebound)";
            "snapshot" => snapshot_infiniband(),
        );
        return Ok(());
    }

    let entries: Vec<_> = match fs::read_dir(ib_dir) {
        Ok(it) => it.flatten().collect(),
        Err(e) => {
            warn!(
                logger,
                "expose_guest_infiniband_devices: read_dir(/dev/infiniband) failed: {e}"
            );
            return Ok(());
        }
    };

    if entries.is_empty() {
        info!(
            logger,
            "expose_guest_infiniband_devices: /dev/infiniband is empty, skipping";
            "snapshot" => snapshot_infiniband(),
        );
        return Ok(());
    }

    struct IbDev {
        path: PathBuf,
        name: String,
        major: i64,
        minor: i64,
        mode: u32,
        uid: u32,
        gid: u32,
    }

    let mut ib_devs: Vec<IbDev> = Vec::new();
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();

        let metadata = match fs::metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                warn!(
                    logger,
                    "expose_guest_infiniband_devices: skipping {} (stat: {e})",
                    path.display()
                );
                continue;
            }
        };

        if !metadata.file_type().is_char_device() {
            continue;
        }

        let rdev = metadata.rdev();
        ib_devs.push(IbDev {
            path,
            name,
            major: stat::major(rdev) as i64,
            minor: stat::minor(rdev) as i64,
            mode: (metadata.mode() & 0o7777) as u32,
            uid: metadata.uid(),
            gid: metadata.gid(),
        });
    }

    // Pass 1: append LinuxDevice entries to spec.linux.devices.
    {
        let linux = spec
            .linux_mut()
            .as_mut()
            .ok_or_else(|| anyhow!("Spec didn't contain linux field"))?;

        for ib in &ib_devs {
            let device = oci::LinuxDeviceBuilder::default()
                .path(ib.path.clone())
                .typ(oci::LinuxDeviceType::C)
                .major(ib.major)
                .minor(ib.minor)
                .file_mode(ib.mode)
                .uid(ib.uid)
                .gid(ib.gid)
                .build()
                .map_err(|e| {
                    anyhow!("failed to build LinuxDevice for {}: {e}", ib.path.display())
                })?;

            if let Some(devices) = linux.devices_mut() {
                let already = devices
                    .iter()
                    .any(|d| d.path().display().to_string() == ib.path.display().to_string());
                if !already {
                    devices.push(device);
                }
            } else {
                linux.set_devices(Some(vec![device]));
            }
        }
    }

    // Pass 2: cgroup allow rules.
    let mut exposed: Vec<String> = Vec::with_capacity(ib_devs.len());
    for ib in &ib_devs {
        let info = DeviceInfo {
            cgroup_type: String::from("c"),
            guest_major: ib.major,
            guest_minor: ib.minor,
        };
        insert_devices_cgroup_rule(logger, spec, &info, true, "rwm")
            .context("insert IB device cgroup rule")?;
        exposed.push(format!(
            "{}({}:{},mode=0o{:o})",
            ib.name, ib.major, ib.minor, ib.mode
        ));
    }

    info!(
        logger,
        "expose_guest_infiniband_devices: injected {} guest IB char device(s)",
        exposed.len();
        "exposed" => exposed.join(","),
        "snapshot" => snapshot_infiniband(),
    );

    Ok(())
}

// Test helper constants for common edge case testing
#[cfg(test)]
pub(crate) mod test_helpers {
    #[cfg(not(target_arch = "s390x"))]
    pub const SUBSYSTEM_BLOCK: &str = "block";
    pub const SUBSYSTEM_NET: &str = "net";
    #[cfg(not(target_arch = "s390x"))]
    pub const ACTION_REMOVE: &str = "remove";
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::linux_abi::create_pci_root_bus_path;
    use crate::uevent::{spawn_test_watcher, wait_for_uevent};
    use oci::{
        Linux, LinuxBuilder, LinuxDeviceBuilder, LinuxDeviceCgroupBuilder, LinuxDeviceType,
        LinuxResources, LinuxResourcesBuilder, SpecBuilder,
    };
    use oci_spec::runtime as oci;
    use rstest::rstest;
    use std::iter::FromIterator;
    use tempfile::tempdir;

    const VM_ROOTFS: &str = "/";
    const TEST_CONTAINER_PATH: &str = "/dev/null";
    const TEST_VM_PATH: &str = "/dev/null";
    const TEST_MAJOR: i64 = 7;
    const TEST_MINOR: i64 = 2;

    // Helper function to create a test logger
    fn create_test_logger() -> slog::Logger {
        slog::Logger::root(slog::Discard, o!())
    }

    // Helper function to create a device update map
    fn create_device_update<'a>(
        container_path: &'a str,
        vm_path: &str,
    ) -> HashMap<&'a str, DevUpdate> {
        HashMap::from_iter(vec![(
            container_path,
            DevUpdate::new(container_path, vm_path).unwrap(),
        )])
    }

    #[rstest]
    #[case::valid_zeros("0000:00:00.0", true)]
    #[case::valid_normal("0000:01:02.3", true)]
    #[case::valid_max("ffff:ff:1f.7", true)]
    #[case::invalid_text("invalid", false)]
    #[case::empty_string("", false)]
    #[case::invalid_format("not_a_pci_address", false)]
    #[case::random_string("random_string", false)]
    #[test]
    fn test_parse_pci_bdf_name(#[case] input: &str, #[case] should_parse: bool) {
        let result = parse_pci_bdf_name(input);
        assert_eq!(
            result.is_some(),
            should_parse,
            "parse_pci_bdf_name('{}') should {} parse",
            input,
            if should_parse {
                "successfully"
            } else {
                "fail to"
            }
        );
    }

    #[rstest]
    #[case::normal(0, 1, 2, 3, "0000:01")]
    #[case::max_values(0xffff, 0xff, 0x1f, 7, "ffff:ff")]
    #[case::all_zeros(0, 0, 0, 0, "0000:00")]
    #[test]
    fn test_bus_of_addr(
        #[case] domain: u16,
        #[case] bus: u8,
        #[case] slot: u8,
        #[case] func: u8,
        #[case] expected: &str,
    ) {
        let addr = pci::Address::new(domain, bus, pci::SlotFn::new(slot, func).unwrap());
        assert_eq!(bus_of_addr(&addr).unwrap(), expected);
    }

    #[rstest]
    #[case::single_bus(
        vec![(0, 1, 0, 0), (0, 1, 1, 0), (0, 1, 2, 0)],
        Some("0000:01")
    )]
    #[case::multiple_buses(
        vec![(0, 1, 0, 0), (0, 2, 0, 0)],
        None
    )]
    #[case::empty_list(vec![], None)]
    #[test]
    fn test_unique_bus_from_pci_addresses(
        #[case] addr_tuples: Vec<(u16, u8, u8, u8)>,
        #[case] expected: Option<&str>,
    ) {
        let addrs: Vec<pci::Address> = addr_tuples
            .into_iter()
            .map(|(d, b, s, f)| pci::Address::new(d, b, pci::SlotFn::new(s, f).unwrap()))
            .collect();

        match expected {
            Some(bus) => assert_eq!(unique_bus_from_pci_addresses(&addrs).unwrap(), bus),
            None => assert!(unique_bus_from_pci_addresses(&addrs).is_err()),
        }
    }

    #[test]
    fn test_read_single_bus_from_pci_bus_dir() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let bridgebuspath = testdir.path().join("pci_bus");
        fs::create_dir_all(&bridgebuspath).unwrap();

        let bus_dir = bridgebuspath.join("0000:01");
        fs::create_dir(&bus_dir).unwrap();
        assert_eq!(
            read_single_bus_from_pci_bus_dir(&bridgebuspath).unwrap(),
            "0000:01"
        );

        let bus_dir2 = bridgebuspath.join("0000:02");
        fs::create_dir(&bus_dir2).unwrap();
        assert!(read_single_bus_from_pci_bus_dir(&bridgebuspath).is_err());

        let empty_dir = testdir.path().join("empty_pci_bus");
        fs::create_dir_all(&empty_dir).unwrap();
        assert!(read_single_bus_from_pci_bus_dir(&empty_dir).is_err());
    }

    #[test]
    fn test_infer_bus_from_child_devices() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let devpath = testdir.path();

        let dev1 = devpath.join("0000:01:00.0");
        let dev2 = devpath.join("0000:01:01.0");
        let dev3 = devpath.join("0000:01:02.0");
        fs::create_dir(&dev1).unwrap();
        fs::create_dir(&dev2).unwrap();
        fs::create_dir(&dev3).unwrap();

        assert_eq!(
            infer_bus_from_child_devices(&devpath.to_path_buf()).unwrap(),
            "0000:01"
        );

        let dev4 = devpath.join("0000:02:00.0");
        fs::create_dir(&dev4).unwrap();
        assert!(infer_bus_from_child_devices(&devpath.to_path_buf()).is_err());

        let empty_dir = testdir.path().join("no_devices");
        fs::create_dir_all(&empty_dir).unwrap();
        assert!(infer_bus_from_child_devices(&empty_dir).is_err());

        let non_pci_dir = testdir.path().join("with_non_pci");
        fs::create_dir_all(&non_pci_dir).unwrap();
        let pci_dev = non_pci_dir.join("0000:03:00.0");
        let non_pci = non_pci_dir.join("not_a_pci_device");
        fs::create_dir(&pci_dev).unwrap();
        fs::create_dir(&non_pci).unwrap();
        assert_eq!(
            infer_bus_from_child_devices(&non_pci_dir).unwrap(),
            "0000:03"
        );
    }

    // valid_zones content → expected value written to the online file.
    // None means the valid_zones file is absent (simulates older kernels or
    // non-memory hotplug sysfs paths).
    #[rstest]
    #[case::movable_only("Movable\n", "online_movable")]
    #[case::normal_and_movable("Normal Movable\n", "online_movable")]
    #[case::normal_only("Normal\n", "1")]
    #[case::empty_file("\n", "1")]
    #[test]
    fn test_online_device_valid_zones(#[case] zones: &str, #[case] expected: &str) {
        let testdir = tempdir().expect("failed to create tmpdir");
        let online_path = testdir.path().join("online");
        let valid_zones_path = testdir.path().join("valid_zones");

        fs::write(&valid_zones_path, zones).unwrap();
        online_device(online_path.to_str().unwrap()).unwrap();
        assert_eq!(
            fs::read_to_string(&online_path).unwrap(),
            expected,
            "valid_zones={zones:?}"
        );
    }

    #[test]
    fn test_online_device_no_valid_zones_file() {
        // No valid_zones present — must fall back to "1" safely.
        let testdir = tempdir().expect("failed to create tmpdir");
        let online_path = testdir.path().join("online");

        online_device(online_path.to_str().unwrap()).unwrap();
        assert_eq!(fs::read_to_string(&online_path).unwrap(), "1");
    }

    #[test]
    fn test_online_device_bad_path_returns_error() {
        assert!(online_device("/nonexistent/path/to/device").is_err());
    }

    #[test]
    fn test_dev_update_new() {
        let result = DevUpdate::new("/dev/null", "/dev/null");
        assert!(result.is_ok());

        let update = result.unwrap();
        assert_eq!(update.final_path, Some("/dev/null".to_string()));

        let result2 = DevUpdate::new("/dev/null", "/dev/custom");
        assert!(result2.is_ok());
        let update2 = result2.unwrap();
        assert_eq!(update2.final_path, Some("/dev/custom".to_string()));

        let result_invalid = DevUpdate::new("/nonexistent/device", "/dev/null");
        assert!(result_invalid.is_err());
    }

    #[rstest]
    #[case::char_device("/dev/null", true, "c", true)]
    #[case::block_device("/", false, "b", true)]
    #[case::nonexistent("/nonexistent/path", true, "", false)]
    #[case::empty_path("", true, "", false)]
    #[test]
    fn test_device_info_new(
        #[case] path: &str,
        #[case] is_char: bool,
        #[case] expected_type: &str,
        #[case] should_succeed: bool,
    ) {
        let result = DeviceInfo::new(path, is_char);

        if should_succeed {
            let info = result.unwrap();
            assert_eq!(info.cgroup_type, expected_type);
            assert!(info.guest_major >= 0);
            assert!(info.guest_minor >= 0);
        } else {
            assert!(result.is_err());
        }
    }

    #[test]
    fn test_spec_update_conversions() {
        let info = DeviceInfo::new("/dev/null", true).unwrap();
        let spec_update: SpecUpdate = info.into();
        assert!(spec_update.dev.is_some());
        assert_eq!(spec_update.pci.len(), 0);

        let dev_update = DevUpdate::new("/dev/null", "/dev/null").unwrap();
        let spec_update2: SpecUpdate = dev_update.into();
        assert!(spec_update2.dev.is_some());
        assert_eq!(spec_update2.pci.len(), 0);

        let spec_update3 = SpecUpdate::default();
        assert!(spec_update3.dev.is_none());
        assert_eq!(spec_update3.pci.len(), 0);
    }

    #[test]
    fn test_cdi_devices_from_visible_devices() {
        let make_spec = |val: Option<&str>| {
            let mut spec = Spec::default();
            if let Some(v) = val {
                let mut process = oci::Process::default();
                *process.env_mut() = Some(vec![format!("VISIBLE_CDI_DEVICES={v}")]);
                spec.set_process(Some(process));
            }
            spec
        };

        assert!(cdi_devices_from_visible_devices(&make_spec(None))
            .expect("Failed to get CDI devices")
            .is_empty());
        for v in ["", "none", "void"] {
            assert!(
                cdi_devices_from_visible_devices(&make_spec(Some(v)))
                    .expect("Failed to get CDI devices")
                    .is_empty(),
                "expected no devices for VISIBLE_CDI_DEVICES={:?}",
                v
            );
        }

        assert_eq!(
            cdi_devices_from_visible_devices(&make_spec(Some("nvidia.com/gpu=all")))
                .expect("Failed to get CDI devices"),
            vec!["nvidia.com/gpu=all".to_string()]
        );

        assert_eq!(
            cdi_devices_from_visible_devices(&make_spec(Some(
                "nvidia.com/gpu=all : nvidia.com/ib=0, 1 ,2"
            )))
            .expect("Failed to get CDI devices"),
            vec![
                "nvidia.com/gpu=all".to_string(),
                "nvidia.com/ib=0".to_string(),
                "nvidia.com/ib=1".to_string(),
                "nvidia.com/ib=2".to_string(),
            ]
        );

        assert!(
            cdi_devices_from_visible_devices(&make_spec(Some("nvidia.com/gpu=0, 1 ,,2"))).is_err()
        );

        for v in ["all", "0,1"] {
            assert!(
                cdi_devices_from_visible_devices(&make_spec(Some(v))).is_err(),
                "expected an error for kind-less VISIBLE_CDI_DEVICES={:?}",
                v
            );
        }

        assert!(cdi_devices_from_visible_devices(&make_spec(Some("=all"))).is_err());
        assert!(cdi_devices_from_visible_devices(&make_spec(Some("nvidia.com/gpu="))).is_err());

        for v in [
            "nvidia.com/gpu=all,0",
            "nvidia.com/gpu=0,all",
            "nvidia.com/gpu=all : nvidia.com/ib=0,all",
        ] {
            assert!(
                cdi_devices_from_visible_devices(&make_spec(Some(v))).is_err(),
                "expected an error for mixed all/index VISIBLE_CDI_DEVICES={:?}",
                v
            );
        }
    }

    #[test]
    fn test_update_device_cgroup() {
        let logger = create_test_logger();
        let mut linux = Linux::default();
        linux.set_resources(Some(LinuxResources::default()));
        let mut spec = SpecBuilder::default().linux(linux).build().unwrap();

        let dev_info = DeviceInfo::new(VM_ROOTFS, false).unwrap();
        insert_devices_cgroup_rule(&logger, &mut spec, &dev_info, false, "rw").unwrap();

        let devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(devices.len(), 1);

        let meta = fs::metadata(VM_ROOTFS).unwrap();
        let rdev = meta.dev();
        let major = stat::major(rdev) as i64;
        let minor = stat::minor(rdev) as i64;

        assert_eq!(devices[0].major(), Some(major));
        assert_eq!(devices[0].minor(), Some(minor));
    }

    #[test]
    fn test_update_spec_devices() {
        let logger = create_test_logger();
        let mut spec = Spec::default();

        // vm_path empty
        let update = DeviceInfo::new("", true);
        assert!(update.is_err());

        // linux is empty
        let res = update_spec_devices(
            &logger,
            &mut spec,
            create_device_update(TEST_CONTAINER_PATH, TEST_VM_PATH),
        );
        assert!(res.is_err());

        spec.set_linux(Some(Linux::default()));

        // linux.devices doesn't contain the updated device
        let res = update_spec_devices(
            &logger,
            &mut spec,
            create_device_update(TEST_CONTAINER_PATH, TEST_VM_PATH),
        );
        assert!(res.is_err());

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .set_devices(Some(vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from("/dev/null2"))
                .major(TEST_MAJOR)
                .minor(TEST_MINOR)
                .build()
                .unwrap()]));

        // guest and host path are not the same
        let res = update_spec_devices(
            &logger,
            &mut spec,
            create_device_update(TEST_CONTAINER_PATH, TEST_VM_PATH),
        );
        assert!(
            res.is_err(),
            "container_path={:?} vm_path={:?} spec={:?}",
            TEST_CONTAINER_PATH,
            TEST_VM_PATH,
            spec
        );

        spec.linux_mut()
            .as_mut()
            .unwrap()
            .devices_mut()
            .as_mut()
            .unwrap()[0]
            .set_path(PathBuf::from(TEST_CONTAINER_PATH));

        // spec.linux.resources is empty
        let res = update_spec_devices(
            &logger,
            &mut spec,
            create_device_update(TEST_CONTAINER_PATH, TEST_VM_PATH),
        );
        assert!(res.is_ok());

        // update both devices and cgroup lists
        spec.linux_mut()
            .as_mut()
            .unwrap()
            .set_devices(Some(vec![LinuxDeviceBuilder::default()
                .path(PathBuf::from(TEST_CONTAINER_PATH))
                .major(TEST_MAJOR)
                .minor(TEST_MINOR)
                .build()
                .unwrap()]));

        spec.linux_mut().as_mut().unwrap().set_resources(Some(
            oci::LinuxResourcesBuilder::default()
                .devices(vec![LinuxDeviceCgroupBuilder::default()
                    .major(TEST_MAJOR)
                    .minor(TEST_MINOR)
                    .build()
                    .unwrap()])
                .build()
                .unwrap(),
        ));

        let res = update_spec_devices(
            &logger,
            &mut spec,
            create_device_update(TEST_CONTAINER_PATH, TEST_VM_PATH),
        );
        assert!(res.is_ok());
    }

    #[test]
    fn test_update_spec_devices_guest_host_conflict() {
        let logger = create_test_logger();

        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let zero_rdev = fs::metadata("/dev/zero").unwrap().rdev();
        let full_rdev = fs::metadata("/dev/full").unwrap().rdev();

        let host_major_a = stat::major(null_rdev) as i64;
        let host_minor_a = stat::minor(null_rdev) as i64;
        let host_major_b = stat::major(zero_rdev) as i64;
        let host_minor_b = stat::minor(zero_rdev) as i64;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/a"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major_a)
                            .minor(host_minor_a)
                            .build()
                            .unwrap(),
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/b"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major_b)
                            .minor(host_minor_b)
                            .build()
                            .unwrap(),
                    ])
                    .resources(
                        LinuxResourcesBuilder::default()
                            .devices(vec![
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major_a)
                                    .minor(host_minor_a)
                                    .build()
                                    .unwrap(),
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major_b)
                                    .minor(host_minor_b)
                                    .build()
                                    .unwrap(),
                            ])
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let container_path_a = "/dev/a";
        let vm_path_a = "/dev/zero";

        let guest_major_a = stat::major(zero_rdev) as i64;
        let guest_minor_a = stat::minor(zero_rdev) as i64;

        let container_path_b = "/dev/b";
        let vm_path_b = "/dev/full";

        let guest_major_b = stat::major(full_rdev) as i64;
        let guest_minor_b = stat::minor(full_rdev) as i64;

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(host_major_a, specdevices[0].major());
        assert_eq!(host_minor_a, specdevices[0].minor());
        assert_eq!(host_major_b, specdevices[1].major());
        assert_eq!(host_minor_b, specdevices[1].minor());

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(host_major_a), specresources_devices[0].major());
        assert_eq!(Some(host_minor_a), specresources_devices[0].minor());
        assert_eq!(Some(host_major_b), specresources_devices[1].major());
        assert_eq!(Some(host_minor_b), specresources_devices[1].minor());

        let updates = HashMap::from_iter(vec![
            (
                container_path_a,
                DeviceInfo::new(vm_path_a, true).unwrap().into(),
            ),
            (
                container_path_b,
                DeviceInfo::new(vm_path_b, true).unwrap().into(),
            ),
        ]);
        let res = update_spec_devices(&logger, &mut spec, updates);
        assert!(res.is_ok());

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(guest_major_a, specdevices[0].major());
        assert_eq!(guest_minor_a, specdevices[0].minor());
        assert_eq!(guest_major_b, specdevices[1].major());
        assert_eq!(guest_minor_b, specdevices[1].minor());

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(guest_major_a), specresources_devices[0].major());
        assert_eq!(Some(guest_minor_a), specresources_devices[0].minor());
        assert_eq!(Some(guest_major_b), specresources_devices[1].major());
        assert_eq!(Some(guest_minor_b), specresources_devices[1].minor());
    }

    #[test]
    fn test_update_spec_devices_char_block_conflict() {
        let logger = create_test_logger();

        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();

        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/char"))
                            .typ(LinuxDeviceType::C)
                            .major(host_major)
                            .minor(host_minor)
                            .build()
                            .unwrap(),
                        LinuxDeviceBuilder::default()
                            .path(PathBuf::from("/dev/block"))
                            .typ(LinuxDeviceType::B)
                            .major(host_major)
                            .minor(host_minor)
                            .build()
                            .unwrap(),
                    ])
                    .resources(
                        LinuxResourcesBuilder::default()
                            .devices(vec![
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::C)
                                    .major(host_major)
                                    .minor(host_minor)
                                    .build()
                                    .unwrap(),
                                LinuxDeviceCgroupBuilder::default()
                                    .typ(LinuxDeviceType::B)
                                    .major(host_major)
                                    .minor(host_minor)
                                    .build()
                                    .unwrap(),
                            ])
                            .build()
                            .unwrap(),
                    )
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let container_path = "/dev/char";
        let vm_path = "/dev/null";

        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(host_major), specresources_devices[0].major());
        assert_eq!(Some(host_minor), specresources_devices[0].minor());
        assert_eq!(Some(host_major), specresources_devices[1].major());
        assert_eq!(Some(host_minor), specresources_devices[1].minor());

        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DeviceInfo::new(vm_path, true).unwrap().into(),
            )]),
        );
        assert!(res.is_ok());

        // Only the char device, not the block device should be updated
        let specresources_devices = spec
            .linux()
            .as_ref()
            .unwrap()
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .clone()
            .unwrap();
        assert_eq!(Some(guest_major), specresources_devices[0].major());
        assert_eq!(Some(guest_minor), specresources_devices[0].minor());
        assert_eq!(Some(host_major), specresources_devices[1].major());
        assert_eq!(Some(host_minor), specresources_devices[1].minor());
    }

    #[test]
    fn test_update_spec_devices_final_path() {
        let logger = create_test_logger();

        let null_rdev = fs::metadata("/dev/null").unwrap().rdev();
        let guest_major = stat::major(null_rdev) as i64;
        let guest_minor = stat::minor(null_rdev) as i64;

        let container_path = "/dev/original";
        let host_major: i64 = 99;
        let host_minor: i64 = 99;

        let mut spec = SpecBuilder::default()
            .linux(
                LinuxBuilder::default()
                    .devices(vec![LinuxDeviceBuilder::default()
                        .path(PathBuf::from(container_path))
                        .typ(LinuxDeviceType::C)
                        .major(host_major)
                        .minor(host_minor)
                        .build()
                        .unwrap()])
                    .build()
                    .unwrap(),
            )
            .build()
            .unwrap();

        let vm_path = "/dev/null";
        let final_path = "/dev/new";

        let res = update_spec_devices(
            &logger,
            &mut spec,
            HashMap::from_iter(vec![(
                container_path,
                DevUpdate::new(vm_path, final_path).unwrap(),
            )]),
        );
        assert!(res.is_ok());

        let specdevices = &spec.linux().as_ref().unwrap().devices().clone().unwrap();
        assert_eq!(guest_major, specdevices[0].major());
        assert_eq!(guest_minor, specdevices[0].minor());
        assert_eq!(&PathBuf::from(final_path), specdevices[0].path());
    }

    #[test]
    fn test_update_env_pci() {
        let example_map = [
            // Each is a host,guest pair of pci addresses
            ("0000:1a:01.0", "0000:01:01.0"),
            ("0000:1b:02.0", "0000:01:02.0"),
            // This one has the same host address as guest address
            // above, to test that we're not double-translating
            ("0000:01:01.0", "ffff:02:1f.7"),
        ];

        let pci_dev_info_original = r#"PCIDEVICE_x_INFO={"0000:1a:01.0":{"generic":{"deviceID":"0000:1a:01.0"}},"0000:1b:02.0":{"generic":{"deviceID":"0000:1b:02.0"}}}"#;
        let pci_dev_info_expected = r#"PCIDEVICE_x_INFO={"0000:01:01.0":{"generic":{"deviceID":"0000:01:01.0"}},"0000:01:02.0":{"generic":{"deviceID":"0000:01:02.0"}}}"#;
        let mut env = vec![
            "PCIDEVICE_x=0000:1a:01.0,0000:1b:02.0".to_string(),
            pci_dev_info_original.to_string(),
            "PCIDEVICE_y=0000:01:01.0".to_string(),
            "NOTAPCIDEVICE_blah=abcd:ef:01.0".to_string(),
        ];

        let _pci_fixups = example_map
            .iter()
            .map(|(h, g)| {
                (
                    pci::Address::from_str(h).unwrap(),
                    pci::Address::from_str(g).unwrap(),
                )
            })
            .collect();

        let cid = "0".to_string();
        let mut pci_fixups: HashMap<String, HashMap<pci::Address, pci::Address>> = HashMap::new();
        pci_fixups.insert(cid.clone(), _pci_fixups);

        let res = update_env_pci(&cid, &mut env, &pci_fixups);
        assert!(res.is_ok(), "error: {}", res.err().unwrap());

        assert_eq!(env[0], "PCIDEVICE_x=0000:01:01.0,0000:01:02.0");
        assert_eq!(env[1], pci_dev_info_expected);
        assert_eq!(env[2], "PCIDEVICE_y=ffff:02:1f.7");
        assert_eq!(env[3], "NOTAPCIDEVICE_blah=abcd:ef:01.0");
    }

    #[test]
    fn test_update_env_pci_non_pci_addresses() {
        let mut env = vec![
            "PCIDEVICE_NVIDIA_COM_BF_SF=mlx5_core.sf.10".to_string(),
            "PCIDEVICE_NVIDIA_COM_BF_SF_INFO={\"mlx5_core.sf.10\":{}}".to_string(),
            "PCIDEVICE_REAL=0000:1a:01.0".to_string(),
        ];

        let example_map = [("0000:1a:01.0", "0000:01:01.0")];
        let _pci_fixups: HashMap<pci::Address, pci::Address> = example_map
            .iter()
            .map(|(h, g)| {
                (
                    pci::Address::from_str(h).unwrap(),
                    pci::Address::from_str(g).unwrap(),
                )
            })
            .collect();

        let cid = "0".to_string();
        let mut pci_fixups: HashMap<String, HashMap<pci::Address, pci::Address>> = HashMap::new();
        pci_fixups.insert(cid.clone(), _pci_fixups);

        let res = update_env_pci(&cid, &mut env, &pci_fixups);
        assert!(res.is_ok(), "error: {}", res.err().unwrap());

        // Non-PCI addresses should be left untouched
        assert_eq!(env[0], "PCIDEVICE_NVIDIA_COM_BF_SF=mlx5_core.sf.10");
        assert_eq!(
            env[1],
            "PCIDEVICE_NVIDIA_COM_BF_SF_INFO={\"mlx5_core.sf.10\":{}}"
        );
        // Real PCI addresses should still be translated
        assert_eq!(env[2], "PCIDEVICE_REAL=0000:01:01.0");
    }

    #[test]
    fn test_update_env_pci_missing_cid_in_pcimap() {
        let mut env = vec![
            "PCIDEVICE_RES=0000:06:02.6".to_string(),
            "PCIDEVICE_RES_INFO={\"0000:06:02.6\":{\"generic\":{\"deviceID\":\"0000:06:02.6\"}}}"
                .to_string(),
            "OTHER=value".to_string(),
        ];
        let pcimap: HashMap<String, HashMap<pci::Address, pci::Address>> = HashMap::new();

        let cid = "container-0".to_string();
        let res = update_env_pci(&cid, &mut env, &pcimap);
        assert!(
            res.is_ok(),
            "must not error when pcimap[cid] missing: {:?}",
            res
        );

        // Both PCIDEVICE_* and *_INFO stay untouched.
        assert_eq!(env[0], "PCIDEVICE_RES=0000:06:02.6");
        assert_eq!(
            env[1],
            "PCIDEVICE_RES_INFO={\"0000:06:02.6\":{\"generic\":{\"deviceID\":\"0000:06:02.6\"}}}"
        );
        assert_eq!(env[2], "OTHER=value");
    }

    #[test]
    fn test_update_env_pci_unknown_host_bdf() {
        let mut env = vec![
            "PCIDEVICE_RES=0000:1a:01.0,0000:99:99.9".to_string(),
            "PCIDEVICE_RES_INFO={\"0000:1a:01.0\":{},\"0000:99:99.9\":{}}".to_string(),
        ];

        let cid = "container-0".to_string();
        let mut inner: HashMap<pci::Address, pci::Address> = HashMap::new();
        inner.insert(
            pci::Address::from_str("0000:1a:01.0").unwrap(),
            pci::Address::from_str("0000:01:01.0").unwrap(),
        );
        let mut pcimap: HashMap<String, HashMap<pci::Address, pci::Address>> = HashMap::new();
        pcimap.insert(cid.clone(), inner);

        let res = update_env_pci(&cid, &mut env, &pcimap);
        assert!(res.is_ok(), "must not error on partial pcimap: {:?}", res);

        // Must leave env vars untouched (no half-translation).
        assert_eq!(env[0], "PCIDEVICE_RES=0000:1a:01.0,0000:99:99.9");
        assert_eq!(
            env[1],
            "PCIDEVICE_RES_INFO={\"0000:1a:01.0\":{},\"0000:99:99.9\":{}}"
        );
    }

    #[test]
    fn test_pcipath_to_sysfs() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let rootbuspath = testdir.path().to_str().unwrap();

        let path2 = pci::Path::from_str("02").unwrap();
        let path23 = pci::Path::from_str("02/03").unwrap();
        let path234 = pci::Path::from_str("02/03/04").unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert!(relpath.is_err());

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files for the device at 0000:00:02.0
        let bridge2path = format!("{}{}", rootbuspath, "/0000:00:02.0");

        fs::create_dir_all(&bridge2path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert!(relpath.is_err());

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files to indicate that 0000:00:02.0 is a bridge to bus 01
        let bridge2bus = "0000:01";
        let bus2path = format!("{bridge2path}/pci_bus/{bridge2bus}");

        fs::create_dir_all(bus2path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert!(relpath.is_err());

        // Create mock sysfs files for a bridge at 0000:01:03.0 to bus 02
        let bridge3path = format!("{bridge2path}/0000:01:03.0");
        let bridge3bus = "0000:02";
        let bus3path = format!("{bridge3path}/pci_bus/{bridge3bus}");

        fs::create_dir_all(bus3path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path2);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");

        let relpath = pcipath_to_sysfs(rootbuspath, &path234);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0/0000:02:04.0");
    }

    #[test]
    fn test_pcipath_to_sysfs_fallback_child_device_scan() {
        let testdir = tempdir().expect("failed to create tmpdir");
        let rootbuspath = testdir.path().to_str().unwrap();

        let path23 = pci::Path::from_str("02/03").unwrap();
        let bridge2path = format!("{}{}", rootbuspath, "/0000:00:02.0");
        let child_device_path = format!("{bridge2path}/0000:01:03.0");

        fs::create_dir_all(child_device_path).unwrap();

        let relpath = pcipath_to_sysfs(rootbuspath, &path23);
        assert_eq!(relpath.unwrap(), "/0000:00:02.0/0000:01:03.0");
    }

    // We use device specific variants of this for real cases, but
    // they have some complications that make them troublesome to unit
    // test
    async fn example_get_device_name(
        sandbox: &Arc<Mutex<Sandbox>>,
        root_complex: &str,
        relpath: &str,
    ) -> Result<String> {
        let matcher =
            crate::device::block_device_handler::VirtioBlkPciMatcher::new(relpath, root_complex);

        let uev = wait_for_uevent(sandbox, matcher).await?;

        Ok(uev.devname)
    }

    #[tokio::test]
    async fn test_get_device_name() {
        let devname = "vda";
        let root_complex = "00";
        let root_bus = create_pci_root_bus_path(root_complex);
        let relpath = "/0000:00:0a.0/0000:03:0b.0";
        let devpath = format!("{root_bus}{relpath}/virtio4/block/{devname}");

        let mut uev = crate::uevent::Uevent::default();
        uev.action = crate::linux_abi::U_EVENT_ACTION_ADD.to_string();
        uev.subsystem = BLOCK.to_string();
        uev.devpath = devpath.clone();
        uev.devname = devname.to_string();

        let logger = create_test_logger();
        let sandbox = Arc::new(Mutex::new(Sandbox::new(&logger).unwrap()));

        let mut sb = sandbox.lock().await;
        sb.uevent_map.insert(devpath.clone(), uev);
        drop(sb); // unlock

        let name = example_get_device_name(&sandbox, root_complex, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), devname);

        let mut sb = sandbox.lock().await;
        let uev = sb.uevent_map.remove(&devpath).unwrap();
        drop(sb); // unlock

        spawn_test_watcher(sandbox.clone(), uev);

        let name = example_get_device_name(&sandbox, root_complex, relpath).await;
        assert!(name.is_ok(), "{}", name.unwrap_err());
        assert_eq!(name.unwrap(), devname);
    }

    #[tokio::test]
    async fn test_handle_cdi_devices() {
        let logger = create_test_logger();
        let mut spec = Spec::default();

        let mut annotations = HashMap::new();
        // cdi.k8s.io/vendor1_devices: vendor1.com/device=foo
        annotations.insert(
            "cdi.k8s.io/vfio17".to_string(),
            "kata.com/gpu=0".to_string(),
        );
        spec.set_annotations(Some(annotations));

        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let cdi_file = temp_dir.path().join("kata.json");

        let cdi_version = "0.6.0";
        let kind = "kata.com/gpu";
        let device_name = "0";
        let annotation_whatever = "false";
        let annotation_whenever = "true";
        let inner_env = "TEST_INNER_ENV=TEST_INNER_ENV_VALUE";
        let outer_env = "TEST_OUTER_ENV=TEST_OUTER_ENV_VALUE";
        let inner_device = "/dev/zero";
        let outer_device = "/dev/null";

        let cdi_content = format!(
            r#"{{
            "cdiVersion": "{cdi_version}",
            "kind": "{kind}",
            "devices": [
                {{
                    "name": "{device_name}",
                    "annotations": {{
                        "whatever": "{annotation_whatever}",
                        "whenever": "{annotation_whenever}"
                    }},
                    "containerEdits": {{
                        "env": [
                            "{inner_env}"
                        ],
                        "deviceNodes": [
                            {{
                                "path": "{inner_device}"
                            }}
                        ]
                    }}
                }}
            ],
            "containerEdits": {{
                "env": [
                    "{outer_env}"
                ],
                "deviceNodes": [
                    {{
                        "path": "{outer_device}"
                    }}
                ]
            }}
        }}"#
        );

        fs::write(&cdi_file, cdi_content).expect("Failed to write CDI file");

        let cdi_timeout = Duration::from_secs(0);

        let res = handle_cdi_devices(
            &logger,
            &mut spec,
            temp_dir.path().to_str().unwrap(),
            cdi_timeout,
            &[],
        )
        .await;
        println!("modfied spec {spec:?}");
        assert!(res.is_ok(), "{}", res.err().unwrap());

        let linux = spec.linux().as_ref().unwrap();
        let devices = linux
            .resources()
            .as_ref()
            .unwrap()
            .devices()
            .as_ref()
            .unwrap();
        assert_eq!(devices.len(), 2);

        let env = spec.process().as_ref().unwrap().env().as_ref().unwrap();

        // find string TEST_OUTER_ENV in env
        let outer_env = env.iter().find(|e| e.starts_with("TEST_OUTER_ENV"));
        assert!(outer_env.is_some(), "TEST_OUTER_ENV not found in env");

        // find TEST_INNER_ENV in env
        let inner_env = env.iter().find(|e| e.starts_with("TEST_INNER_ENV"));
        assert!(inner_env.is_some(), "TEST_INNER_ENV not found in env");
    }

    #[test]
    fn test_unsatisfiable_cdi_devices() {
        let available = vec![
            "kata.com/gpu=0".to_string(),
            "kata.com/gpu=1".to_string(),
            "kata.com/gpu=all".to_string(),
        ];

        // Devices the kind provides are satisfiable.
        for d in ["kata.com/gpu=0", "kata.com/gpu=1", "kata.com/gpu=all"] {
            assert!(
                unsatisfiable_cdi_devices(&available, &[d.to_string()]).is_empty(),
                "{} should be satisfiable",
                d
            );
        }

        // A device the (present) kind does not provide is unsatisfiable.
        assert_eq!(
            unsatisfiable_cdi_devices(&available, &["kata.com/gpu=5".to_string()]),
            vec!["kata.com/gpu=5".to_string()]
        );

        // A kind that is entirely absent is left for the caller to wait on.
        assert!(
            unsatisfiable_cdi_devices(&available, &["other.com/gpu=0".to_string()]).is_empty(),
            "an absent kind must not be reported as unsatisfiable"
        );

        // Only the offending device of a mixed request is reported.
        assert_eq!(
            unsatisfiable_cdi_devices(
                &available,
                &[
                    "kata.com/gpu=0".to_string(),
                    "kata.com/gpu=5".to_string(),
                    "other.com/gpu=0".to_string(),
                ]
            ),
            vec!["kata.com/gpu=5".to_string()]
        );
    }

    async fn run_handle_cdi_devices(
        spec_dir: &str,
        cdi_timeout: Duration,
        extra_devices: &[String],
    ) -> Result<()> {
        let logger = slog::Logger::root(slog::Discard, o!());
        let mut spec = Spec::default();
        handle_cdi_devices(&logger, &mut spec, spec_dir, cdi_timeout, extra_devices).await
    }

    #[tokio::test]
    async fn test_handle_cdi_devices_missing_device_of_known_kind() {
        // A CDI spec providing kind "kata.com/gpu" with only devices "0" and "1".
        let temp_dir = tempdir().expect("Failed to create temporary directory");
        let spec_dir = temp_dir.path().to_str().unwrap();
        let cdi_content = r#"{
            "cdiVersion": "0.6.0",
            "kind": "kata.com/gpu",
            "devices": [
                { "name": "0", "containerEdits": { "deviceNodes": [{ "path": "/dev/null" }] } },
                { "name": "1", "containerEdits": { "deviceNodes": [{ "path": "/dev/null" }] } }
            ]
        }"#;
        fs::write(temp_dir.path().join("kata.json"), cdi_content)
            .expect("Failed to write CDI file");

        // The kind is present but device "5" is not. Use a generous timeout: if
        // the fail-fast path is broken the test would otherwise hang for the
        // full duration before reporting a timeout instead.
        let res = run_handle_cdi_devices(
            spec_dir,
            Duration::from_secs(30),
            &["kata.com/gpu=5".to_string()],
        )
        .await;
        let msg = res
            .expect_err("missing device of a known kind must be rejected")
            .to_string();
        assert!(
            msg.contains("kata.com/gpu=5"),
            "error should name the missing device, got: {}",
            msg
        );
        assert!(
            !msg.contains("CDI timeout"),
            "should fail fast, not via the wait/timeout path, got: {}",
            msg
        );

        // An entirely absent kind goes through the wait path and ultimately
        // times out (cdi_timeout = 0 means a single attempt).
        let res = run_handle_cdi_devices(
            spec_dir,
            Duration::from_secs(0),
            &["absent.com/gpu=0".to_string()],
        )
        .await;
        let msg = res
            .expect_err("an unsatisfied absent kind must eventually time out")
            .to_string();
        assert!(
            msg.contains("CDI timeout"),
            "an absent kind should take the wait/timeout path, got: {}",
            msg
        );
    }
}
