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
stability_dir="$(dirname "$(readlink -f "$0")")"
source "${stability_dir}/../common.bash"

function install_dependencies() {
	info "Installing the dependencies needed for running the containerd-stability tests"

	declare -a system_deps=(
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq

	declare -a github_deps
	github_deps[0]="cri_containerd:$(get_from_kata_deps "externals.containerd.${CONTAINERD_VERSION}")"

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		install_${dep[0]} "${dep[1]}"
	done
}

function run() {
	info "Running soak parallel stability tests using ${KATA_HYPERVISOR} hypervisor"

	export ITERATIONS=2 MAX_CONTAINERS=20
	bash "${stability_dir}/soak_parallel_rm.sh"

	info "Running scability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/scability_test.sh" 15 60

#	info "Running agent stability test using ${KATA_HYPERVISOR} hypervisor"
#	bash "${stability_dir}/agent_stability_test.sh"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		install-kata) install_kata ;;
		enabling-hypervisor) enabling_hypervisor ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
