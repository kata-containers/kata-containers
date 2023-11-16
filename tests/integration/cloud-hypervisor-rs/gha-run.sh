#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

kata_tarball_dir="${2:-kata-artifacts}"
gral_dir="$(dirname "$(readlink -f "$0")")"
source "${gral_dir}/../../common.bash"

declare -r cloud_hypervisor_rs_dir="${gral_dir}/../../../src/runtime-rs"
declare -r stability_dir="${gral_dir}/../../stability"

function install_cloud_hypervisor_rs() {
	info "Installing the cloud hypervisor runtime-rs"
	pushd "${cloud_hypervisor_rs_dir}"
	HYPERVISOR=cloud-hypervisor make
	sudo make install
	popd
}

function setup_cloud_hypervisor_rs() {
	info "Verify that the shimv2 runtime-rs is installed"
	/usr/local/bin/containerd-shim-kata-v2 --version | grep -i rust

	info "Use the cloud hypervisor runtime-rs configuration"
	sudo ln -sf /usr/shared/defaults/kata-containers/cloud-hypervisor-configuration.toml /usr/shared/defaults/kata-containers/configuration.toml
	sudo systemctl daemon-reload

	info "Verify cloud hypervisor runtime-rs configuration"
	kata-ctl env
}

function install_dependencies() {
	info "Installing the dependencies needed for running the tests"

	declare -a system_deps=(
		curl
		gnupg
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq
	install_docker
}

function run() {
	# This will be enable once that the general setup and installation is done properly
	info "Running soak parallel stability tests using ${KATA_HYPERVISOR} hypervisor"
#	export ITERATIONS=2 MAX_CONTAINERS=20
#	bash "${stability_dir}/soak_parallel_rm.sh"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		install-kata) install_kata ;;
		install-cloud-hypervisor-rs) install_cloud_hypervisor_rs ;;
		setup-cloud-hypervisor-rs) setup_cloud_hypervisor_rs ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
