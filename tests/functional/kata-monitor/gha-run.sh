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
export kata_tarball_dir
kata_monitor_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
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

	# Dependency list of projects that we can install
	# directly from their releases on GitHub:
	# - containerd
	#   - cri-container-cni release tarball already includes CNI plugins
	# - runc
	# - cni-plugins
	declare -a github_deps
	# shellcheck disable=SC2154
	github_deps[0]="cri_containerd:$(get_from_kata_deps ".externals.containerd.${CONTAINERD_VERSION}")"
	github_deps[1]="runc:$(get_from_kata_deps ".externals.runc.latest")"
	github_deps[2]="cni_plugins:$(get_from_kata_deps ".externals.cni-plugins.version")"

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		"install_${dep[0]}" "${dep[1]}"
	done

	# cri-tools is resolved at install time to the absolute latest
	# release, so it is not pinned via versions.yaml.
	install_cri_tools
}

function run() {
	# shellcheck disable=SC2154
	info "Running cri-containerd tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor
	bash "${kata_monitor_dir}/kata-monitor-tests.sh"
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
