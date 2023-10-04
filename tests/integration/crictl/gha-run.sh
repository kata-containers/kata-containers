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
script_dir="$(dirname "$(readlink -f "$0")")" 
source "${script_dir}/../../common.bash"

function install_dependencies() {
	info "Installing the dependencies needed for running the cri-containerd tests"

	# Dependency list of projects that we can rely on the system packages
	# - build-essential
	#   - Theoretically we only need `make`, but doesn't hurt to install
	#     the whole build-essential group
	# - jq
	declare -a system_deps=(
		build-essential
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq
	${repo_root_dir}/tests/install_go.sh -p

	# Dependency list of projects that we can install them
	# directly from their releases on GitHub:
	# - containerd
	#   - cri-container-cni release tarball already includes CNI plugins
	# - cri-tools
	declare -a github_deps
	github_deps[0]="cri_containerd:$(get_from_kata_deps "externals.containerd.${CONTAINERD_VERSION}")"
	github_deps[1]="cri_tools:$(get_from_kata_deps "externals.critools.latest")"

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		install_${dep[0]} "${dep[1]}"
	done

	# Clone containerd as we'll need to build it in order to run the tests
	# base_version: The version to be intalled in the ${major}.${minor} format
	clone_cri_containerd $(get_from_kata_deps "externals.containerd.${CONTAINERD_VERSION}")
}

function run() {
	info "Running crictl tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor
	bash -c ${script_dir}/run_tests.sh
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		install-kata) install_kata ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
