// Copyright (c) 2018 Intel Corporation
// Copyright (c) 2018 HyperHQ Inc.
//
// SPDX-License-Identifier: Apache-2.0
//
// Note that some variables are "var" to allow them to be modified
// by the tests.

package katautils

var defaultHypervisorPath = "/usr/bin/qemu-lite-system-x86_64"
var defaultImagePath = "/usr/share/kata-containers/kata-containers.img"
var defaultKernelPath = "/usr/share/kata-containers/vmlinuz.container"
var defaultInitrdPath = "/usr/share/kata-containers/kata-containers-initrd.img"
var defaultFirmwarePath = ""
var defaultMachineAccelerators = ""
var defaultShimPath = "/usr/libexec/kata-containers/kata-shim"
var systemdUnitName = "kata-containers.target"

const defaultKernelParams = ""
const defaultMachineType = "pc"

const defaultVCPUCount uint32 = 1
const defaultMaxVCPUCount uint32 = 0
const defaultMemSize uint32 = 2048 // MiB
const defaultMemSlots uint32 = 10
const defaultMemOffset uint32 = 0 // MiB
const defaultBridgesCount uint32 = 1
const defaultInterNetworkingModel = "macvtap"
const defaultDisableBlockDeviceUse bool = false
const defaultBlockDeviceDriver = "virtio-scsi"
const defaultBlockDeviceCacheSet bool = false
const defaultBlockDeviceCacheDirect bool = false
const defaultBlockDeviceCacheNoflush bool = false
const defaultEnableIOThreads bool = false
const defaultEnableMemPrealloc bool = false
const defaultEnableHugePages bool = false
const defaultFileBackedMemRootDir string = ""
const defaultEnableSwap bool = false
const defaultEnableDebug bool = false
const defaultDisableNestingChecks bool = false
const defaultMsize9p uint32 = 8192
const defaultHotplugVFIOOnRootBus bool = false
const defaultEntropySource = "/dev/urandom"
const defaultGuestHookPath string = ""

const defaultTemplatePath string = "/run/vc/vm/template"
const defaultVMCacheEndpoint string = "/var/run/kata-containers/cache.sock"

// Default config file used by stateless systems.
var defaultRuntimeConfiguration = "/usr/share/defaults/kata-containers/configuration.toml"

// Alternate config file that takes precedence over
// defaultRuntimeConfiguration.
var defaultSysConfRuntimeConfiguration = "/etc/kata-containers/configuration.toml"

var name = "kata"
var defaultProxyPath = "/usr/libexec/kata-containers/kata-proxy"
var defaultNetmonPath = "/usr/libexec/kata-containers/kata-netmon"
