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

# The repository NVIDIA publishes per distro tag (26.04 -> ubuntu2604) and per
# arch ('sbsa' for arm64) is pinned in versions.yaml under
# externals.nvidia.cuda.repo, the same entry the GPU rootfs builds consume. The
# devkit shell has no versions.yaml to read, so the build bakes it in here.
base_url="@CUDA_REPO_URL@"
keyring_deb="@CUDA_REPO_PKG@"

if [[ -z "${base_url}" ]] || [[ "${base_url}" == @* ]] || [[ -z "${keyring_deb}" ]]; then
	echo "add-nvidia-repos: no CUDA repository was baked into this devkit (see externals.nvidia.cuda.repo in versions.yaml)" >&2
	exit 1
fi

# versions.yaml stores the repository URL with a trailing slash.
base_url="${base_url%/}"

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
