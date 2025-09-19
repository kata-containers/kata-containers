// Copyright 2025 Kata Contributors
//
// SPDX-License-Identifier: Apache-2.0
//

/// Q35 machine type identifier (default for x86_64 virtualization).
/// Used by QEMU for PCI-based virtio devices.
pub const MACHINE_TYPE_Q35_TYPE: &str = "q35";

/// S390x CCW virtio machine type identifier.
/// Used on IBM Z architecture for channel I/O (CCW) virtio devices.
pub const MACHINE_TYPE_S390X_TYPE: &str = "s390-ccw-virtio";
