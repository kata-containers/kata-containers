#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

[[ $# == 2 ]] || { echo "Usage: $0 TARBALL DESTINATION" >&2; exit 1; }
archive=$(realpath "$1")
sha256sum "${archive}"
mkdir -p "$2"
destination=$(realpath "$2")
work=$(mktemp -d)
trap 'rm -rf "${work}"' EXIT
tar --zstd -xf "${archive}" -C "${work}"
install -m 0644 "${work}/opt/kata/share/kata-containers/kata-containers-substrate.img" "${destination}/rootfs.img"
install -m 0644 "${work}/opt/kata/share/kata-containers/vmlinux.container" "${destination}/vmlinux"
(cd "${destination}"; sha256sum rootfs.img vmlinux > SHA256SUMS)
