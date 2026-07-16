#!/usr/bin/env bash
#
# Copyright (c) 2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

kata_packaging_dir="${script_dir}/../.."
kata_packaging_scripts="${kata_packaging_dir}/scripts"

kata_static_build_dir="${kata_packaging_dir}/static-build"
kata_static_build_scripts="${kata_static_build_dir}/scripts"

ARCH=${ARCH:-$(uname -m)}

QEMU_REPO="${QEMU_REPO:-}"
QEMU_VERSION_NUM="${QEMU_VERSION_NUM:-}"
QEMU_TARBALL="${QEMU_TARBALL:-}"
QEMU_DESTDIR="${QEMU_DESTDIR:-}"
HYPERVISOR_NAME="${HYPERVISOR_NAME:-}"
PREFIX="${PREFIX:-}"
PKGVERSION=${PKGVERSION:-}

workdir="$(mktemp -d)"
trap 'rm -rf "${workdir}"' EXIT
git clone --depth=1 "${QEMU_REPO}" "${workdir}/qemu"
pushd "${workdir}/qemu"
git fetch --depth=1 origin "${QEMU_VERSION_NUM}"
git checkout FETCH_HEAD
scripts/git-submodule.sh update meson capstone
"${kata_packaging_scripts}"/patch_qemu.sh "${QEMU_VERSION_NUM}" "${kata_packaging_dir}/qemu/patches"

# With --without-default-devices every machine type and every device with
# "default y" is suppressed (allnoconfig semantics).  We must explicitly
# list each CONFIG_* we need before ./configure so meson picks them up.
# The post-build verify_devices check below fails the build if any device
# Kata can emit is missing from the resulting binary.
#
# Transport-independent device models used by Kata on every architecture
# (the PCI/CCW transport variant is built when the transport is enabled):
#   VIRTIO_BLK  – rootfs / container block device
#   VIRTIO_SCSI – SCSI block driver option (also selects SCSI for scsi-hd)
#   VIRTIO_NET  – container networking (accelerated by vhost-kernel)
#   VIRTIO_SERIAL / VIRTIO_CONSOLE – agent tty / console
#   VIRTIO_RNG  – guest entropy source
#   VIRTIO_BALLOON – memory pressure management
#   VIRTIO_MEM  – memory hot-plug
#   VHOST_USER_FS – virtiofsd shared filesystem (vhost-user)
#   VHOST_USER_BLK – vhost-user block (SPDK and friends)
#   VHOST_USER_SCSI – vhost-user SCSI
#   VIRTIO_9P   – fallback 9P shared filesystem
#   VHOST_VSOCK – VM↔host socket for the Kata agent (kernel vhost)

_COMMON_DEVS='
CONFIG_VIRTIO_BLK=y
CONFIG_VIRTIO_SCSI=y
CONFIG_VIRTIO_NET=y
CONFIG_VIRTIO_SERIAL=y
CONFIG_VIRTIO_RNG=y
CONFIG_VIRTIO_BALLOON=y
CONFIG_VIRTIO_MEM=y
CONFIG_VHOST_USER_FS=y
CONFIG_VHOST_USER_BLK=y
CONFIG_VHOST_USER_SCSI=y
CONFIG_VIRTIO_9P=y
CONFIG_VHOST_VSOCK=y
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
#   NVDIMM      – rootfs image as DAX-capable persistent memory

_PCIE_DEVS='
CONFIG_VIRTIO_PCI=y
CONFIG_VFIO_PCI=y
CONFIG_IOMMUFD=y
CONFIG_PCIE_PORT=y
CONFIG_XIO3130=y
CONFIG_PCI_BRIDGE=y
CONFIG_PCIE_PCI_BRIDGE=y
CONFIG_PXB=y
CONFIG_NVDIMM=y
'

if [[ "${ARCH}" == "x86_64" ]]; then
	# VTD_ACCEL enables IOMMUFD-backed VT-d for high-performance passthrough.
	# PVPANIC_ISA provides the pvpanic device (guest kernel panic reporting).
	printf 'CONFIG_Q35=y\n%s\n%s\nCONFIG_VTD=y\nCONFIG_VTD_ACCEL=y\nCONFIG_AMD_IOMMU=y\nCONFIG_PVPANIC_ISA=y\n' \
		"${_COMMON_DEVS}" "${_PCIE_DEVS}" \
		>> configs/devices/i386-softmmu/default.mak
elif [[ "${ARCH}" == "s390x" ]]; then
	# s390x uses CCW bus (no PCI virtio); VIRTIO_CCW replaces VIRTIO_PCI and
	# selects VIRTIO_MD_SUPPORTED.  Passthrough is via VFIO_CCW / VFIO_AP.
	# None of the PCIe topology devices apply.
	printf 'CONFIG_S390_CCW_VIRTIO=y\n%s\nCONFIG_VIRTIO_CCW=y\nCONFIG_VFIO_CCW=y\nCONFIG_VFIO_AP=y\n' \
		"${_COMMON_DEVS}" \
		>> configs/devices/s390x-softmmu/default.mak
elif [[ "${ARCH}" == "aarch64" ]]; then
	# CONFIG_CXL is required by CONFIG_ACPI_CXL (auto-selected by ARM_VIRT) for Rubin vCXL.
	# CONFIG_PXB (in _PCIE_DEVS) is a dependency of CONFIG_CXL.
	# arm-smmuv3 comes via ARM_VIRT (selects ARM_SMMUV3).
	printf 'CONFIG_ARM_VIRT=y\nCONFIG_CXL=y\nCONFIG_CXL_MEM_DEVICE=y\n%s\n%s\n' \
		"${_COMMON_DEVS}" "${_PCIE_DEVS}" \
		>> configs/devices/aarch64-softmmu/default.mak
elif [[ "${ARCH}" == "ppc64le" ]]; then
	# VIRTIO_MEM depends on VIRTIO_MEM_SUPPORTED, which is only selected on
	# arm/i386/s390x — ppc64 PSeries does not support virtio-mem.
	# PSeries PHBs are conventional PCI: no PCIe root/switch ports, no PXB.
	_PPC64_DEVS=$(printf '%s' "${_COMMON_DEVS}" | grep -v 'CONFIG_VIRTIO_MEM=')
	printf 'CONFIG_PSERIES=y\n%s\nCONFIG_VIRTIO_PCI=y\nCONFIG_VFIO_PCI=y\nCONFIG_IOMMUFD=y\nCONFIG_PCI_BRIDGE=y\nCONFIG_NVDIMM=y\n' \
		"${_PPC64_DEVS}" \
		>> configs/devices/ppc64-softmmu/default.mak
	unset _PPC64_DEVS
fi
unset _COMMON_DEVS _PCIE_DEVS

if [[ "$(uname -m)" != "${ARCH}" ]] && [[ "${ARCH}" == "s390x" ]]; then
       PREFIX="${PREFIX}" "${kata_packaging_scripts}"/configure-hypervisor.sh -s "${HYPERVISOR_NAME}" "${ARCH}" | xargs ./configure  --with-pkgversion="${PKGVERSION}" --cc=s390x-linux-gnu-gcc --cross-prefix=s390x-linux-gnu- --prefix="${PREFIX}" --target-list=s390x-softmmu
else
       PREFIX="${PREFIX}" "${kata_packaging_scripts}"/configure-hypervisor.sh -s "${HYPERVISOR_NAME}" "${ARCH}" | xargs ./configure  --with-pkgversion="${PKGVERSION}"
fi

# Build only the system emulator, not tests or tools.
# ppc64le uses a "ppc64" target name; all others match the arch.
_qemu_target="${ARCH}"
[[ "${ARCH}" == "ppc64le" ]] && _qemu_target="ppc64"
make -j"$(nproc --ignore=1)" "qemu-system-${_qemu_target}"

# Fail the build if any device Kata can emit is missing from the binary.
# This is the completeness guarantee for the --without-default-devices
# allowlist above: a gap fails here at build time, not at VM-start time.
# Skipped when cross-compiling (the binary cannot run on the build host).
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

_pci_devs=(
	virtio-blk-pci virtio-scsi-pci scsi-hd virtio-net-pci
	virtio-serial-pci virtconsole virtio-rng-pci virtio-balloon-pci
	virtio-9p-pci vhost-vsock-pci vhost-user-fs-pci vhost-user-blk-pci
	nvdimm vfio-pci pci-bridge
)
_pcie_topology=(
	pcie-root-port x3130-upstream xio3130-downstream pcie-pci-bridge pxb-pcie
	virtio-mem-pci
)

if [[ "$(uname -m)" == "${ARCH}" ]]; then
	case "${ARCH}" in
	x86_64)
		verify_devices "${_pci_devs[@]}" "${_pcie_topology[@]}" \
			intel-iommu amd-iommu pvpanic
		;;
	aarch64)
		verify_devices "${_pci_devs[@]}" "${_pcie_topology[@]}" \
			arm-smmuv3
		;;
	ppc64le)
		verify_devices "${_pci_devs[@]}"
		;;
	s390x)
		verify_devices \
			virtio-blk-ccw virtio-scsi-ccw scsi-hd virtio-net-ccw \
			virtio-serial-ccw virtconsole virtio-rng-ccw virtio-balloon-ccw \
			virtio-9p-ccw vhost-vsock-ccw vhost-user-fs-ccw virtio-mem-ccw \
			vfio-ccw vfio-ap
		;;
	esac
else
	echo "verify_devices: skipped (cross-compiling for ${ARCH} on $(uname -m))"
fi
unset _pci_devs _pcie_topology _qemu_target

make install DESTDIR="${QEMU_DESTDIR}"
popd
"${kata_static_build_scripts}"/qemu-build-post.sh
mv "${QEMU_DESTDIR}/${QEMU_TARBALL}" /share/
