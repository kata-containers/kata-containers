#!/usr/bin/env bash
# Copyright (c) Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Per-architecture allowlists for merged release tarballs:
#   kata-static.tar.zst  - runtime-rs focused (composable guest images)
#   kata-go-static.tar.zst - Go runtime focused (monolithic guest images)

set -o errexit
set -o nounset
set -o pipefail

normalize_arch() {
	case "${1}" in
		x86_64|amd64) echo "amd64" ;;
		aarch64|arm64) echo "arm64" ;;
		s390x) echo "s390x" ;;
		ppc64le) echo "ppc64le" ;;
		*) echo "unsupported architecture: ${1}" >&2; return 1 ;;
	esac
}

kata_static_tarball_inputs() {
	local arch
	arch="$(normalize_arch "${1}")"

	case "${arch}" in
		amd64)
			cat <<'EOF'
kata-static-cloud-hypervisor.tar.zst
kata-static-kernel-debug.tar.zst
kata-static-kernel-dragonball-experimental.tar.zst
kata-static-kernel-nvidia-gpu.tar.zst
kata-static-kernel.tar.zst
kata-static-nydus.tar.zst
kata-static-openvmm.tar.zst
kata-static-ovmf-sev.tar.zst
kata-static-ovmf-tdx.tar.zst
kata-static-ovmf.tar.zst
kata-static-qemu-no-shared-fs.tar.zst
kata-static-qemu-microvm.tar.zst
kata-static-qemu-snp-experimental.tar.zst
kata-static-qemu-tdx-experimental.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-image-coco-extension.tar.zst
kata-static-rootfs-image-devkit-extension.tar.zst
kata-static-rootfs-image-mariner.tar.zst
kata-static-rootfs-image-nvidia-gpu-extension.tar.zst
kata-static-rootfs-image-nvidia.tar.zst
kata-static-rootfs-image.tar.zst
kata-static-rootfs-initrd-confidential.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-rust.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
		arm64)
			cat <<'EOF'
kata-static-cloud-hypervisor.tar.zst
kata-static-kernel-debug.tar.zst
kata-static-kernel-dragonball-experimental.tar.zst
kata-static-kernel-nvidia-gpu.tar.zst
kata-static-kernel.tar.zst
kata-static-nydus.tar.zst
kata-static-openvmm.tar.zst
kata-static-ovmf.tar.zst
kata-static-qemu-no-shared-fs.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-image-coco-extension.tar.zst
kata-static-rootfs-image-devkit-extension.tar.zst
kata-static-rootfs-image-nvidia-gpu-extension.tar.zst
kata-static-rootfs-image-nvidia.tar.zst
kata-static-rootfs-image.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-rust.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
		s390x)
			cat <<'EOF'
kata-static-boot-image-se-runtime-rs.tar.zst
kata-static-kernel.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-image-coco-extension.tar.zst
kata-static-rootfs-image.tar.zst
kata-static-rootfs-initrd-confidential.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-rust.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
		ppc64le)
			cat <<'EOF'
kata-static-kernel.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-rust.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
	esac
}

kata_go_tarball_inputs() {
	local arch
	arch="$(normalize_arch "${1}")"

	case "${arch}" in
		amd64)
			cat <<'EOF'
kata-static-cloud-hypervisor.tar.zst
kata-static-firecracker.tar.zst
kata-static-kernel-debug.tar.zst
kata-static-kernel-nvidia-gpu.tar.zst
kata-static-kernel.tar.zst
kata-static-nydus.tar.zst
kata-static-ovmf-sev.tar.zst
kata-static-ovmf-tdx.tar.zst
kata-static-ovmf.tar.zst
kata-static-qemu-snp-experimental.tar.zst
kata-static-qemu-tdx-experimental.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-image-confidential.tar.zst
kata-static-rootfs-image-mariner.tar.zst
kata-static-rootfs-image-nvidia-gpu-confidential.tar.zst
kata-static-rootfs-image-nvidia-gpu.tar.zst
kata-static-rootfs-image-nvidia.tar.zst
kata-static-rootfs-image.tar.zst
kata-static-rootfs-initrd-confidential.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-go.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
		arm64)
			cat <<'EOF'
kata-static-cloud-hypervisor.tar.zst
kata-static-firecracker.tar.zst
kata-static-kernel-debug.tar.zst
kata-static-kernel-nvidia-gpu.tar.zst
kata-static-kernel.tar.zst
kata-static-nydus.tar.zst
kata-static-ovmf.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-image-confidential.tar.zst
kata-static-rootfs-image-nvidia-gpu.tar.zst
kata-static-rootfs-image-nvidia.tar.zst
kata-static-rootfs-image.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-go.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
		s390x)
			cat <<'EOF'
kata-static-boot-image-se.tar.zst
kata-static-kernel.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-image-confidential.tar.zst
kata-static-rootfs-image.tar.zst
kata-static-rootfs-initrd-confidential.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-go.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
		ppc64le)
			cat <<'EOF'
kata-static-kernel.tar.zst
kata-static-qemu.tar.zst
kata-static-rootfs-initrd.tar.zst
kata-static-shim-v2-go.tar.zst
kata-static-virtiofsd.tar.zst
EOF
			;;
	esac
}

if [[ "${BASH_SOURCE[0]}" == "${0}" ]]; then
	action="${1:-}"
	arch="${2:-$(uname -m)}"
	case "${action}" in
		kata-static) kata_static_tarball_inputs "${arch}" ;;
		kata-go-static) kata_go_tarball_inputs "${arch}" ;;
		*) echo "usage: ${0} {kata-static|kata-go-static} [arch]" >&2; exit 1 ;;
	esac
fi
