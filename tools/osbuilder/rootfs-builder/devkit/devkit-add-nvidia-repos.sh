#!/bin/bash
#
# Copyright (c) Kata Containers Community
#
# SPDX-License-Identifier: Apache-2.0
#
# Run from inside the devkit debug shell (i.e. already in the overlay/chroot) to
# add NVIDIA's CUDA apt repository, so `apt-get install <pkg>` can then pull
# NVIDIA userspace (nvidia-utils, cuda-toolkit, ...) on demand. Kept out of the
# prebaked toolset to keep the generic devkit image small and vendor-neutral.
set -euo pipefail

if [[ ! -r /etc/os-release ]]; then
	echo "add-nvidia-repos: /etc/os-release missing; run me inside the devkit shell" >&2
	exit 1
fi

# shellcheck disable=SC1091
. /etc/os-release
if [[ "${ID:-}" != "ubuntu" ]]; then
	echo "add-nvidia-repos: expected an Ubuntu devkit (got ID='${ID:-}')" >&2
	exit 1
fi

# NVIDIA publishes repos per distro tag (24.04 -> ubuntu2404) and per arch, with
# 'sbsa' standing in for arm64.
version_id="${VERSION_ID:-}"
distro="ubuntu${version_id//./}"
case "$(uname -m)" in
	x86_64) repo_arch="x86_64" ;;
	aarch64) repo_arch="sbsa" ;;
	*) echo "add-nvidia-repos: unsupported arch $(uname -m)" >&2; exit 1 ;;
esac

base_url="https://developer.download.nvidia.com/compute/cuda/repos/${distro}/${repo_arch}"
keyring_deb="cuda-keyring_1.1-1_all.deb"

echo "add-nvidia-repos: adding ${base_url}"
tmpdir="$(mktemp -d)"
trap 'rm -rf "${tmpdir}"' EXIT

# The cuda-keyring package drops the signed-by GPG key and the sources.list entry
# NVIDIA expects, so a plain apt-get update trusts the repo afterwards.
curl -fsSL -o "${tmpdir}/${keyring_deb}" "${base_url}/${keyring_deb}"
dpkg -i "${tmpdir}/${keyring_deb}"
apt-get update

cat <<'EOM'
add-nvidia-repos: NVIDIA CUDA repository ready.
Install packages on demand, e.g.:
  apt-get install -y nvidia-utils-<version>   # provides nvidia-smi
  apt-get install -y cuda-toolkit
EOM
