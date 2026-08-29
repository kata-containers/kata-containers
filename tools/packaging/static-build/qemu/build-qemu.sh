#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

# Environment variables passed from container
QEMU_REPO="${QEMU_REPO:-}"
QEMU_VERSION_NUM="${QEMU_VERSION_NUM:-}"
HYPERVISOR_NAME="${HYPERVISOR_NAME:-}"
PKGVERSION="${PKGVERSION:-}"
PREFIX="${PREFIX:-}"
QEMU_DESTDIR="${QEMU_DESTDIR:-}"
QEMU_TARBALL="${QEMU_TARBALL:-}"

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

kata_packaging_dir="${script_dir}/../.."
kata_packaging_scripts="${kata_packaging_dir}/scripts"

kata_static_build_dir="${kata_packaging_dir}/static-build"
kata_static_build_scripts="${kata_static_build_dir}/scripts"

ARCH=${ARCH:-$(uname -m)}

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT
git clone --depth=1 "${QEMU_REPO}" "${workdir}/qemu"
pushd "${workdir}/qemu"
git fetch --depth=1 origin "${QEMU_VERSION_NUM}"
git checkout FETCH_HEAD
scripts/git-submodule.sh update meson capstone
"${kata_packaging_scripts}/patch_qemu.sh" "${QEMU_VERSION_NUM}" "${kata_packaging_dir}/qemu/patches"

# With --without-default-devices every machine type and every device with
# "default y" is suppressed (allnoconfig semantics).  We must explicitly
# list each CONFIG_* we need before ./configure so meson picks them up.
# The post-build verify_devices check below fails the build if any device
# Kata can emit is missing from the resulting binary.
#
# The no-shared-fs tarball (make qemu-no-shared-fs-tarball) narrows the
# allowlist further: it is built from the same QEMU, but only for the runtime
# classes that boot a block rootfs with shared_fs="none" and never resize the
# guest (enable_virtio_mem and reclaim_guest_freed_memory are both off).
# Memory hot-plug, the balloon, a shared filesystem, a DAX rootfs, a vIOMMU and
# the confidential-guest backends are all left out of it, and only x86_64 and
# aarch64 build it.  Device assignment is untouched: vfio-pci, IOMMUFD and the
# whole PCIe topology (_PCIE_DEVS below) stay in.
#
# The snp-experimental and tdx-experimental tarballs get that same trim plus
# the confidential-guest backends: those classes run with shared_fs="none"
# too, a confidential guest cannot boot an nvdimm rootfs, and neither of them
# resizes the guest.  SGX is not part of it, as nothing emits an EPC backend
# on a TEE guest.
#
# The microvm tarball takes the same trim further and swaps q35 and the PCI
# transport for microvm and virtio-mmio, dropping the mandatory SeaBIOS stage
# and most of the DSDT the guest interprets.  Having no PCI bus it cannot host
# vfio-pci, so any class that passes a device through needs the q35 build.
# x86_64 only: QEMU's MICROVM depends on I386.
_no_shared_fs=false
_tee=false
_microvm=false
case "${HYPERVISOR_NAME}" in
kata-qemu-no-shared-fs)
	_no_shared_fs=true
	;;
kata-qemu-microvm)
	_no_shared_fs=true
	_microvm=true
	;;
kata-qemu-snp-experimental | kata-qemu-tdx-experimental)
	_no_shared_fs=true
	_tee=true
	;;
esac
if [[ "${_no_shared_fs}" == "true" ]] && [[ "${ARCH}" != "x86_64" ]] && [[ "${ARCH}" != "aarch64" ]]; then
	echo "ERROR: ${HYPERVISOR_NAME} is only built for x86_64 and aarch64, not ${ARCH}" >&2
	exit 1
fi
if [[ "${_microvm}" == "true" ]] && [[ "${ARCH}" != "x86_64" ]]; then
	echo "ERROR: ${HYPERVISOR_NAME} is only built for x86_64, not ${ARCH}" >&2
	exit 1
fi

# Transport-independent device models used by Kata on every architecture
# (the PCI/CCW transport variant is built when the transport is enabled):
#   VIRTIO_BLK  – rootfs / container block device
#   VIRTIO_SCSI – SCSI block driver option (also selects SCSI for scsi-hd)
#   VIRTIO_NET  – container networking (accelerated by vhost-kernel)
#   VIRTIO_SERIAL / VIRTIO_CONSOLE – agent tty / console
#   VIRTIO_RNG  – guest entropy source
#   VHOST_USER_BLK – vhost-user block (SPDK and friends)
#   VHOST_USER_SCSI – vhost-user SCSI
#   VHOST_VSOCK – VM↔host socket for the Kata agent (kernel vhost)

_COMMON_DEVS='
CONFIG_VIRTIO_BLK=y
CONFIG_VIRTIO_SCSI=y
CONFIG_VIRTIO_NET=y
CONFIG_VIRTIO_SERIAL=y
CONFIG_VIRTIO_RNG=y
CONFIG_VHOST_USER_BLK=y
CONFIG_VHOST_USER_SCSI=y
CONFIG_VHOST_VSOCK=y
'

# VIRTIO_MEM – memory hot-plug (enable_virtio_mem).  It depends on
# VIRTIO_MEM_SUPPORTED, which is only selected on arm, i386 and s390x, so ppc64
# PSeries cannot compose it.

_MEM_DEVS='
CONFIG_VIRTIO_MEM=y
'

# VIRTIO_BALLOON – reclaim of guest-freed memory (reclaim_guest_freed_memory).
# A group of its own, and not part of _MEM_DEVS, because ppc64 supports the
# balloon but not virtio-mem.

_BALLOON_DEVS='
CONFIG_VIRTIO_BALLOON=y
'

# The shared filesystem, also transport-independent:
#   VHOST_USER_FS – virtiofsd shared filesystem (vhost-user)
#   VIRTIO_9P   – fallback 9P shared filesystem

_SHARED_FS_DEVS='
CONFIG_VHOST_USER_FS=y
CONFIG_VIRTIO_9P=y
'

# PCI transport plus the PCIe slot topology Kata builds for device
# assignment (cold-plug root ports, switch ports, bridges, expander
# bridges for NUMA-pinned GPU complexes):
#   VIRTIO_PCI  – PCI transport for all virtio devices above
#   VFIO_PCI    – GPU / NIC passthrough via VFIO (also selects VFIO)
#   IOMMUFD     – modern VFIO backend (depends on VFIO)
#   PCIE_PORT   – pcie-root-port (hot-plug slots, root-port topology)
#   XIO3130     – x3130-upstream / xio3130-downstream (switch-port topology)
#   PCI_BRIDGE  – pci-bridge (bridge-port topology)
#   PCIE_PCI_BRIDGE – pcie-pci-bridge
#   PXB         – pxb-pcie expander bridge (NUMA-pinned GPU root complexes)

_PCIE_DEVS='
CONFIG_VIRTIO_PCI=y
CONFIG_VFIO_PCI=y
CONFIG_IOMMUFD=y
CONFIG_PCIE_PORT=y
CONFIG_XIO3130=y
CONFIG_PCI_BRIDGE=y
CONFIG_PCIE_PCI_BRIDGE=y
CONFIG_PXB=y
'

# NVDIMM – rootfs image as DAX-capable persistent memory

_DAX_DEVS='
CONFIG_NVDIMM=y
'

# x86_64 only.  VTD_ACCEL enables IOMMUFD-backed VT-d for high-performance
# passthrough:

_IOMMU_DEVS='
CONFIG_VTD=y
CONFIG_VTD_ACCEL=y
CONFIG_AMD_IOMMU=y
'

# x86_64 only.  TDX and SEV are only implied by CONFIG_PC, so allnoconfig drops
# them and the -object types the runtimes emit for confidential guests
# (tdx-guest, sev-snp-guest) are gone from the binary.  CONFIG_SEV is QEMU's
# shared gate for both classic SEV and SEV-SNP; Kata only uses the SNP object.

_TEE_DEVS='
CONFIG_TDX=y
CONFIG_SEV=y
'

# x86_64 only.  SGX backs the memory-backend-epc object the Go runtime emits
# for the sgx.intel.com/epc resource.  A group of its own, and not part of
# _TEE_DEVS, because the confidential-guest builds keep the TEE backends but
# have no use for an enclave.

_SGX_DEVS='
CONFIG_SGX=y
'

# x86_64 microvm build only.  MICROVM selects VIRTIO_MMIO itself; the transport
# is listed so the allowlist states it.  Replaces CONFIG_Q35 and _PCIE_DEVS.

_MICROVM_DEVS='
CONFIG_MICROVM=y
CONFIG_VIRTIO_MMIO=y
'

if [[ "${_no_shared_fs}" == "true" ]]; then
	_MEM_DEVS=
	_BALLOON_DEVS=
	_SHARED_FS_DEVS=
	_DAX_DEVS=
	_IOMMU_DEVS=
	_SGX_DEVS=
	[[ "${_tee}" == "true" ]] || _TEE_DEVS=
fi

if [[ "${ARCH}" == "x86_64" ]] && [[ "${_microvm}" == "true" ]]; then
	# No CONFIG_Q35 and no _PCIE_DEVS: with no PCI bus the *-pci models are
	# dead weight and vfio-pci is out of scope.  microvm has an ISA bus, so
	# PVPANIC_ISA still applies.
	printf 'CONFIG_MICROVM=y\n%s\n%s\nCONFIG_PVPANIC_ISA=y\n' \
		"${_COMMON_DEVS}" "${_MICROVM_DEVS}" \
		>> configs/devices/i386-softmmu/default.mak
elif [[ "${ARCH}" == "x86_64" ]]; then
	# PVPANIC_ISA provides the pvpanic device (guest kernel panic reporting).
	# CONFIG_CXL is required because CONFIG_PXB (in _PCIE_DEVS) links against
	# CXL component symbols; omitting it produces undefined-reference link errors.
	printf 'CONFIG_Q35=y\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\n%s\nCONFIG_PVPANIC_ISA=y\nCONFIG_CXL=y\nCONFIG_CXL_MEM_DEVICE=y\n' \
		"${_COMMON_DEVS}" "${_MEM_DEVS}" "${_BALLOON_DEVS}" \
		"${_SHARED_FS_DEVS}" "${_PCIE_DEVS}" \
		"${_DAX_DEVS}" "${_IOMMU_DEVS}" "${_TEE_DEVS}" "${_SGX_DEVS}" \
		>> configs/devices/i386-softmmu/default.mak
elif [[ "${ARCH}" == "s390x" ]]; then
	# s390x uses CCW bus (no PCI virtio); VIRTIO_CCW replaces VIRTIO_PCI and
	# selects VIRTIO_MD_SUPPORTED.  Passthrough is via VFIO_CCW / VFIO_AP.
	# VFIO_PCI is still required: the VFIO core references its symbols.
	printf 'CONFIG_S390_CCW_VIRTIO=y\n%s\n%s\n%s\n%s\nCONFIG_VIRTIO_CCW=y\nCONFIG_VFIO_CCW=y\nCONFIG_VFIO_AP=y\nCONFIG_VFIO_PCI=y\n' \
		"${_COMMON_DEVS}" "${_MEM_DEVS}" "${_BALLOON_DEVS}" \
		"${_SHARED_FS_DEVS}" \
		>> configs/devices/s390x-softmmu/default.mak
elif [[ "${ARCH}" == "aarch64" ]]; then
	# CONFIG_CXL is required by CONFIG_ACPI_CXL (auto-selected by ARM_VIRT) for Rubin vCXL.
	# CONFIG_PXB (in _PCIE_DEVS) is a dependency of CONFIG_CXL.
	# arm-smmuv3 comes via ARM_VIRT (selects ARM_SMMUV3).
	printf 'CONFIG_ARM_VIRT=y\nCONFIG_CXL=y\nCONFIG_CXL_MEM_DEVICE=y\n%s\n%s\n%s\n%s\n%s\n%s\n' \
		"${_COMMON_DEVS}" "${_MEM_DEVS}" "${_BALLOON_DEVS}" \
		"${_SHARED_FS_DEVS}" "${_PCIE_DEVS}" "${_DAX_DEVS}" \
		>> configs/devices/aarch64-softmmu/default.mak
elif [[ "${ARCH}" == "ppc64le" ]]; then
	# PSeries composes neither _MEM_DEVS (no VIRTIO_MEM_SUPPORTED) nor the
	# PCIe topology: its PHBs are conventional PCI, with no root/switch
	# ports and no PXB.
	printf 'CONFIG_PSERIES=y\n%s\n%s\n%s\nCONFIG_VIRTIO_PCI=y\nCONFIG_VFIO_PCI=y\nCONFIG_IOMMUFD=y\nCONFIG_PCI_BRIDGE=y\n%s\n' \
		"${_COMMON_DEVS}" "${_BALLOON_DEVS}" "${_SHARED_FS_DEVS}" \
		"${_DAX_DEVS}" \
		>> configs/devices/ppc64-softmmu/default.mak
fi
unset _COMMON_DEVS _MEM_DEVS _BALLOON_DEVS _SHARED_FS_DEVS _PCIE_DEVS
unset _DAX_DEVS _IOMMU_DEVS _TEE_DEVS _SGX_DEVS _MICROVM_DEVS

PREFIX="${PREFIX}" "${kata_packaging_scripts}/configure-hypervisor.sh" -s "${HYPERVISOR_NAME}" "${ARCH}" | xargs ./configure  --with-pkgversion="${PKGVERSION}"

# Build only the system emulator, not tests or tools.
# ppc64le uses a "ppc64" target name; all others match the arch.
_qemu_target="${ARCH}"
[[ "${ARCH}" == "ppc64le" ]] && _qemu_target="ppc64"
make -j"$(nproc --ignore=1)" "qemu-system-${_qemu_target}"

# Fail the build if any device Kata can emit is missing from the binary.
# This is the completeness guarantee for the --without-default-devices
# allowlist above: a gap fails here at build time, not at VM-start time.
verify_devices() {
	local qemu_binary="./build/qemu-system-${_qemu_target}"
	local devices_help missing=()
	devices_help=$("${qemu_binary}" -device help)
	local dev
	for dev in "$@"; do
		if ! grep -q "name \"${dev}\"" <<< "${devices_help}"; then
			missing+=("${dev}")
		fi
	done
	if [[ "${#missing[@]}" -gt 0 ]]; then
		echo "ERROR: required devices missing from ${qemu_binary}: ${missing[*]}" >&2
		exit 1
	fi
	echo "verify_devices: all $# required devices present in ${qemu_binary}"
}

# Same guarantee for the user-creatable objects (-object) Kata emits: the
# confidential-guest and SGX backends are objects, not devices, so they do
# not show up in -device help.
verify_objects() {
	local qemu_binary="./build/qemu-system-${_qemu_target}"
	local objects_help missing=()
	objects_help=$("${qemu_binary}" -object help)
	local obj
	for obj in "$@"; do
		if ! grep -q "^[[:space:]]*${obj}$" <<< "${objects_help}"; then
			missing+=("${obj}")
		fi
	done
	if [[ "${#missing[@]}" -gt 0 ]]; then
		echo "ERROR: required objects missing from ${qemu_binary}: ${missing[*]}" >&2
		exit 1
	fi
	echo "verify_objects: all $# required objects present in ${qemu_binary}"
}

_pci_devs=(
	virtio-blk-pci virtio-scsi-pci scsi-hd virtio-net-pci
	virtio-serial-pci virtconsole virtio-rng-pci
	vhost-vsock-pci vhost-user-blk-pci vfio-pci pci-bridge
)
_pcie_topology=(
	pcie-root-port x3130-upstream xio3130-downstream pcie-pci-bridge pxb-pcie
)
# Mirrors the grouping of the allowlist above.
_mem_devs=(virtio-mem-pci)
_balloon_devs=(virtio-balloon-pci)
_shared_fs_devs=(virtio-9p-pci vhost-user-fs-pci)
_dax_devs=(nvdimm)
_iommu_devs=(intel-iommu amd-iommu)
_tee_objs=(tdx-guest sev-snp-guest)
if [[ "${_no_shared_fs}" == "true" ]]; then
	_mem_devs=()
	_balloon_devs=()
	_shared_fs_devs=()
	_dax_devs=()
	_iommu_devs=()
	[[ "${_tee}" == "true" ]] || _tee_objs=()
fi

# virtio-mmio counterparts of _pci_devs.  scsi-hd is shared with it: container
# block devices are LUNs on a cold-plugged virtio-scsi controller, which is how
# block hotplug works without a PCI bus.  Upstream naming is not uniform -- the
# virtio models take a "-device" suffix, the vhost-user ones do not.
_mmio_devs=(
	virtio-blk-device virtio-scsi-device scsi-hd virtio-net-device
	virtio-serial-device virtconsole virtio-rng-device
	vhost-vsock-device vhost-user-blk vhost-user-scsi
)

case "${ARCH}" in
x86_64)
	if [[ "${_microvm}" == "true" ]]; then
		verify_devices "${_mmio_devs[@]}" pvpanic
	else
		verify_devices "${_pci_devs[@]}" "${_mem_devs[@]}" "${_balloon_devs[@]}" \
			"${_shared_fs_devs[@]}" "${_dax_devs[@]}" "${_pcie_topology[@]}" \
			"${_iommu_devs[@]}" pvpanic
		if [[ "${#_tee_objs[@]}" -gt 0 ]]; then
			verify_objects "${_tee_objs[@]}"
		fi
	fi
	;;
aarch64)
	verify_devices "${_pci_devs[@]}" "${_mem_devs[@]}" "${_balloon_devs[@]}" \
		"${_shared_fs_devs[@]}" "${_dax_devs[@]}" "${_pcie_topology[@]}" \
		arm-smmuv3
	;;
ppc64le)
	verify_devices "${_pci_devs[@]}" "${_balloon_devs[@]}" \
		"${_shared_fs_devs[@]}" "${_dax_devs[@]}"
	;;
s390x)
	verify_devices \
		virtio-blk-ccw virtio-scsi-ccw scsi-hd virtio-net-ccw \
		virtio-serial-ccw virtconsole virtio-rng-ccw virtio-balloon-ccw \
		virtio-9p-ccw vhost-vsock-ccw vhost-user-fs-ccw virtio-mem-ccw \
		vfio-ccw vfio-ap
	;;
esac
unset _pci_devs _pcie_topology _mem_devs _balloon_devs _shared_fs_devs
unset _dax_devs _iommu_devs _tee_objs _mmio_devs
unset _no_shared_fs _tee _microvm _qemu_target

make install DESTDIR="${QEMU_DESTDIR}"
popd
"${kata_static_build_scripts}/qemu-build-post.sh"
mv "${QEMU_DESTDIR}/${QEMU_TARBALL}" /share/
