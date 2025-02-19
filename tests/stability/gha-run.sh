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
source "${stability_dir}/../metrics/lib/common.bash"
DOCKERFILE="${stability_dir}/stressng_dockerfile/Dockerfile"
IMAGE="docker.io/library/local-stressng:latest"

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
	check_ctr_images "${IMAGE}" "${DOCKERFILE}"
}

function run() {
	info "Running soak parallel stability tests using ${KATA_HYPERVISOR} hypervisor"

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
