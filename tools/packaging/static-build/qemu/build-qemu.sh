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

rm -rf qemu
git clone --depth=1 "${QEMU_REPO}" qemu
pushd qemu
git fetch --depth=1 origin "${QEMU_VERSION_NUM}"
git checkout FETCH_HEAD
scripts/git-submodule.sh update meson capstone
"${kata_packaging_scripts}/patch_qemu.sh" "${QEMU_VERSION_NUM}" "${kata_packaging_dir}/qemu/patches"

# With --without-default-devices every machine type and every device with
# "default y" is suppressed (allnoconfig semantics).  We must explicitly
# list each CONFIG_* we need before ./configure so meson picks them up.
#
# Common virtio devices used by Kata on every architecture:
#   VIRTIO_BLK  – rootfs / container block device
#   VIRTIO_NET  – container networking (accelerated by vhost-kernel)
#   VIRTIO_SERIAL / VIRTIO_CONSOLE – agent tty / console
#   VIRTIO_RNG  – guest entropy source
#   VIRTIO_BALLOON – memory pressure management
#   VIRTIO_MEM  – memory hot-plug
#   VHOST_USER_FS – virtiofsd shared filesystem (vhost-user)
#   VIRTIO_9P   – fallback 9P shared filesystem
#   VHOST_VSOCK – VM↔host socket for the Kata agent (kernel vhost)
#   VFIO_PCI    – GPU / NIC passthrough via VFIO (also selects VFIO)
#   IOMMUFD     – modern VFIO backend (depends on VFIO)

_COMMON_DEVS='
CONFIG_VIRTIO_PCI=y
CONFIG_VIRTIO_BLK=y
CONFIG_VIRTIO_NET=y
CONFIG_VIRTIO_SERIAL=y
CONFIG_VIRTIO_RNG=y
CONFIG_VIRTIO_BALLOON=y
CONFIG_VIRTIO_MEM=y
CONFIG_VHOST_USER_FS=y
CONFIG_VIRTIO_9P=y
CONFIG_VHOST_VSOCK=y
CONFIG_VFIO_PCI=y
CONFIG_IOMMUFD=y
'

if [[ "${ARCH}" == "x86_64" ]]; then
	# VTD_ACCEL enables IOMMUFD-backed VT-d for high-performance passthrough.
	printf 'CONFIG_Q35=y\n%s\nCONFIG_VTD=y\nCONFIG_VTD_ACCEL=y\nCONFIG_AMD_IOMMU=y\n' "${_COMMON_DEVS}" \
		>> configs/devices/i386-softmmu/default.mak
elif [[ "${ARCH}" == "s390x" ]]; then
	# s390x uses CCW bus (no PCI virtio); VIRTIO_CCW replaces VIRTIO_PCI and
	# selects VIRTIO_MD_SUPPORTED.  Passthrough is via VFIO_CCW / VFIO_AP.
	# IOMMUFD and VFIO_PCI are not applicable on s390x.
	_S390_DEVS=$(printf '%s' "${_COMMON_DEVS}" | grep -v 'VIRTIO_PCI\|VFIO_PCI\|IOMMUFD')
	printf 'CONFIG_S390_CCW_VIRTIO=y\n%s\nCONFIG_VIRTIO_CCW=y\nCONFIG_VFIO_CCW=y\nCONFIG_VFIO_AP=y\n' "${_S390_DEVS}" \
		>> configs/devices/s390x-softmmu/default.mak
elif [[ "${ARCH}" == "aarch64" ]]; then
	# CONFIG_CXL is required by CONFIG_ACPI_CXL (auto-selected by ARM_VIRT) for Rubin vCXL.
	# CONFIG_PXB is a dependency of CONFIG_CXL.
	printf 'CONFIG_ARM_VIRT=y\nCONFIG_PXB=y\nCONFIG_CXL=y\nCONFIG_CXL_MEM_DEVICE=y\n%s\n' \
		"${_COMMON_DEVS}" >> configs/devices/aarch64-softmmu/default.mak
elif [[ "${ARCH}" == "ppc64le" ]]; then
	# VIRTIO_MEM depends on VIRTIO_MEM_SUPPORTED, which is only selected on
	# arm/i386/s390x — ppc64 PSeries does not support virtio-mem.
	_PPC64_DEVS=$(printf '%s' "${_COMMON_DEVS}" | grep -v 'VIRTIO_MEM\b')
	printf 'CONFIG_PSERIES=y\n%s\n' "${_PPC64_DEVS}" \
		>> configs/devices/ppc64-softmmu/default.mak
	unset _PPC64_DEVS
fi
unset _COMMON_DEVS _S390_DEVS

if [[ "$(uname -m)" != "${ARCH}" ]] && [[ "${ARCH}" == "s390x" ]]; then
       PREFIX="${PREFIX}" "${kata_packaging_scripts}/configure-hypervisor.sh" -s "${HYPERVISOR_NAME}" "${ARCH}" | xargs ./configure  --with-pkgversion="${PKGVERSION}" --cc=s390x-linux-gnu-gcc --cross-prefix=s390x-linux-gnu- --prefix="${PREFIX}" --target-list=s390x-softmmu
else
       PREFIX="${PREFIX}" "${kata_packaging_scripts}/configure-hypervisor.sh" -s "${HYPERVISOR_NAME}" "${ARCH}" | xargs ./configure  --with-pkgversion="${PKGVERSION}"
fi
# Build only the system emulator, not tests or tools.
# ppc64le uses a "ppc64" target name; all others match the arch.
_qemu_target="${ARCH}"
[[ "${ARCH}" == "ppc64le" ]] && _qemu_target="ppc64"
make -j"$(nproc --ignore=1)" "qemu-system-${_qemu_target}"
unset _qemu_target
make install DESTDIR="${QEMU_DESTDIR}"
popd
"${kata_static_build_scripts}/qemu-build-post.sh"
mv "${QEMU_DESTDIR}/${QEMU_TARBALL}" /share/
