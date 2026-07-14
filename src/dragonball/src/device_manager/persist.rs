// Copyright 2026 Ant Group. All rights reserved.
// SPDX-License-Identifier: Apache-2.0

//! Shared snapshot state and helpers for virtio devices (see
//! `crate::snapshot`).

use std::sync::Arc;

use dbs_device::DeviceIo;
use dbs_virtio_devices::persist::{MmioV2TransportState, VirtioDeviceInfoState};
use dbs_virtio_devices::Error as VirtioError;
use serde_derive::{Deserialize, Serialize};

use super::DbsMmioV2Device;
use dbs_virtio_devices::persist::VirtioDevicePersist;

/// Transport-specific snapshot state of a virtio device.
///
/// The device-class config and the guest-negotiated
/// [`VirtioDeviceInfoState`] are transport-independent; only this part
/// differs between MMIO- and (future) PCI-attached devices.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum VirtioTransportState {
    /// virtio-mmio (MMIO v2) transport state.
    Mmio(MmioV2TransportState),
    // TODO: Pci(VirtioPciTransportState) — dbs_pci has no snapshot support
    // yet: it needs serde mirrors for VirtioPciCommonConfig (feature selects,
    // per-queue enable and ring addresses), MsixState (table, PBA, per-vector
    // masking) and BAR programming, plus activation replay on restore. Until
    // then `save_device_state`/`restore_device_state` refuse PCI-attached
    // devices via `VirtioError::InvalidInput` rather than silently dropping
    // that state.
}

/// Common snapshot state for a virtio device, independent of class and
/// transport: static configuration `C`, the guest-negotiated device
/// state, and the transport state.
///
/// Compatibility policy (see `crate::snapshot`): only append new fields,
/// with `#[serde(default)]`; never remove or repurpose existing ones.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct VirtioDevState<C, S = VirtioDeviceInfoState> {
    /// Static configuration of the device.
    pub config: C,
    /// Device state, as captured by the device's own [`VirtioDevicePersist`]
    /// impl. Defaults to [`VirtioDeviceInfoState`], which is all the device
    /// classes snapshotted today need; a device with durable state of its own
    /// (a balloon's size, virtio-mem's region map) names its own type here.
    pub device_info: S,
    /// Transport state.
    pub transport: VirtioTransportState,
}

/// Capture the device and transport state of a virtio device of concrete
/// type `D`, whichever transport it is attached to.
///
/// Dispatches on the transport: MMIO is supported; PCI-attached devices are
/// refused (see [`VirtioTransportState`]).
pub(crate) fn save_device_state<'a, D>(
    device: &Arc<dyn DeviceIo>,
    args: D::SaveArgs,
) -> std::result::Result<(D::State, VirtioTransportState), VirtioError>
where
    D: VirtioDevicePersist<'a, Error = VirtioError> + 'static,
{
    // Transport dispatch. A device that is neither MMIO- nor (once
    // supported) PCI-attached is refused rather than silently skipped.
    let mmio_dev = device
        .as_any()
        .downcast_ref::<DbsMmioV2Device>()
        .ok_or(VirtioError::InvalidInput)?;
    let transport = mmio_dev.save_state();
    let mut guard = mmio_dev.state();
    let inner = guard
        .get_inner_device_mut()
        .as_any_mut()
        .downcast_mut::<D>()
        .ok_or(VirtioError::InvalidInput)?;
    Ok((
        inner.save_state(args)?,
        VirtioTransportState::Mmio(transport),
    ))
}

/// Restore the device and transport state of a freshly re-created virtio
/// device of concrete type `D`, replaying device activation if the guest
/// had activated it. Must be called before the vCPUs resume.
///
/// Dispatches on the transport, like [`save_device_state`].
pub(crate) fn restore_device_state<'a, D>(
    device: &Arc<dyn DeviceIo>,
    device_info: &D::State,
    transport: &VirtioTransportState,
    args: D::RestoreArgs,
) -> std::result::Result<(), VirtioError>
where
    D: VirtioDevicePersist<'a, Error = VirtioError> + 'static,
{
    // Transport dispatch, mirroring `save_device_state`.
    let VirtioTransportState::Mmio(transport) = transport;
    let mmio_dev = device
        .as_any()
        .downcast_ref::<DbsMmioV2Device>()
        .ok_or(VirtioError::InvalidInput)?;
    {
        let mut guard = mmio_dev.state();
        let inner = guard
            .get_inner_device_mut()
            .as_any_mut()
            .downcast_mut::<D>()
            .ok_or(VirtioError::InvalidInput)?;
        // Restore the guest-negotiated device state before the
        // transport state: activation replay reads it.
        inner.restore_state(device_info, args)?;
    }
    mmio_dev.restore_state(transport)
}
