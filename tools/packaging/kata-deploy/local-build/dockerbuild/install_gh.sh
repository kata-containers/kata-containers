#!/bin/bash
#
# Copyright (c) 2026 Kata Containers contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# Install the GitHub CLI, used to verify the provenance attestation of the
# guest-components CoCo extension image (see kata-deploy-binaries.sh).

set -o errexit
set -o nounset
set -o pipefail

install_dest="/usr/local/bin"

# Keep in sync with a version that ships "gh attestation verify".
gh_required_version="2.62.0"

if command -v gh &>/dev/null; then
	echo "gh is already installed in the system"
	exit 0
fi

arch=$(uname -m)
case "${arch}" in
	x86_64) arch="amd64" ;;
	aarch64) arch="arm64" ;;
	s390x)
		# The GitHub CLI publishes no linux/s390x build, so provenance
		# verification is skipped on s390x by the caller.
		echo "gh CLI has no s390x build; skipping installation"
		exit 0
		;;
	*)
		echo "Unsupported architecture for gh CLI: ${arch}"
		exit 1
		;;
esac

gh_tarball="gh_${gh_required_version}_linux_${arch}.tar.gz"

echo "Downloading gh ${gh_required_version}"
tmp_dir="$(mktemp -d)"
curl -fsSL -o "${tmp_dir}/${gh_tarball}" \
	"https://github.com/cli/cli/releases/download/v${gh_required_version}/${gh_tarball}"

echo "Installing gh to ${install_dest}"
tar -C "${tmp_dir}" -xzf "${tmp_dir}/${gh_tarball}"
sudo install -D --mode 0755 \
	"${tmp_dir}/gh_${gh_required_version}_linux_${arch}/bin/gh" \
	"${install_dest}/gh"
rm -rf "${tmp_dir}"
