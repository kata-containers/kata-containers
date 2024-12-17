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
nydus_dir="$(dirname "$(readlink -f "$0")")"
source "${nydus_dir}/../../common.bash"

function install_dependencies() {
	info "Installing the dependencies needed for running the nydus tests"

	# Dependency list of projects that we can rely on the system packages
	# - jq
	declare -a system_deps=(
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq

	# Dependency list of projects that we can install them
	# directly from their releases on GitHub:
	# - containerd
	#   - cri-container-cni release tarball already includes CNI plugins
	# - cri-tools
	# - nydus
	# - nydus-snapshotter
	declare -a github_deps
	github_deps[0]="cri_containerd:$(get_from_kata_deps ".externals.containerd.${CONTAINERD_VERSION}")"
	github_deps[1]="cri_tools:$(get_from_kata_deps ".externals.critools.latest")"
	github_deps[2]="nydus:$(get_from_kata_deps ".externals.nydus.version")"
	github_deps[3]="nydus_snapshotter:$(get_from_kata_deps ".externals.nydus-snapshotter.version")"
	github_deps[4]="runc:$(get_from_kata_deps ".externals.runc.latest")"
	github_deps[5]="cni_plugins:$(get_from_kata_deps ".externals.cni-plugins.version")"

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		install_${dep[0]} "${dep[1]}"
	done
}

function run() {
	info "Running nydus tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor
	bash -c "${nydus_dir}/nydus_tests.sh"
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
