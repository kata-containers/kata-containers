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
		curl
		gnupg
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq
	install_docker
}

function run() {
	info "Running soak parallel stability tests using ${KATA_HYPERVISOR} hypervisor"

	if [ "${KATA_HYPERVISOR}" = "dragonball" ]; then
		echo "Skipping test for ${KATA_HYPERVISOR}"
		return 0
	fi

	export ITERATIONS=2 MAX_CONTAINERS=20
	bash "${stability_dir}/soak_parallel_rm.sh"

	info "Running stressng scability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/stressng.sh"

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
