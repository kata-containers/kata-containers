#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_name="$(basename "${BASH_SOURCE[0]}")"

source "${script_dir}/common.bash"

install_dest="/usr/local/bin"

function get_installed_oras_version() {
	oras version | grep Version | sed -e s/Version:// | tr -d [:blank:]
}

ensure_yq

oras_required_version=$(get_from_kata_deps "externals.oras.version")
if command -v oras; then
	if [[ "${oras_required_version}" == "v$(get_installed_oras_version)" ]]; then
		info "ORAS is already installed in the system"
		exit 0
	fi

	info "Proceeding to cleanup the previous installed version of ORAS, and install the version specified in the versions.yaml file"
	oras_system_path=$(which oras)
	sudo rm -f ${oras_system_path}
fi

goarch=$("${repo_root_dir}/tests/kata-arch.sh" --golang)
oras_tarball="oras_${oras_required_version#v}_linux_${goarch}.tar.gz"
oras_url=$(get_from_kata_deps "externals.oras.url")

info "Downloading ORAS ${oras_required_version}"
curl -OL ${oras_url}/releases/download/${oras_required_version}/${oras_tarball}

info "Installing ORAS to ${install_dest}"
sudo mkdir -p "${install_dest}"
sudo tar -C "${install_dest}" -xzf "${oras_tarball}"
