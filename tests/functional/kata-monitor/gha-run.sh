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
kata_monitor_dir="$(dirname "$(readlink -f "$0")")" 
source "${kata_monitor_dir}/../../common.bash"

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

	# Dependency list of projects that we can install them
	# directly from their releases on GitHub:
	# - cri-tools
	# - containerd
	#   - cri-container-cni release tarball already includes CNI plugins
	cri_tools_version=$(get_from_kata_deps ".externals.critools.latest")
	declare -a github_deps
	github_deps[0]="cri_tools:${cri_tools_version}"
	case "${CONTAINER_ENGINE}" in
		containerd)
			github_deps[1]="cri_containerd:$(get_from_kata_deps ".externals.containerd.${CONTAINERD_VERSION}")"
			github_deps[2]="runc:$(get_from_kata_deps ".externals.runc.latest")"
			github_deps[3]="cni_plugins:$(get_from_kata_deps ".externals.cni-plugins.version")"
			;;
		crio)
			github_deps[1]="cni_plugins:$(get_from_kata_deps ".externals.cni-plugins.version")"
			;;
	esac

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		install_${dep[0]} "${dep[1]}"
	done

	if [ "${CONTAINER_ENGINE}" = "crio" ]; then
		install_crio ${cri_tools_version#v}
	fi
}

function run() {
	info "Running cri-containerd tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor
	bash -c ${kata_monitor_dir}/kata-monitor-tests.sh
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
