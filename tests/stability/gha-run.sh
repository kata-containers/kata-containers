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

function run() {
	info "Running soak parallel stability tests using ${KATA_HYPERVISOR} hypervisor"

	# export ITERATIONS=2 MAX_CONTAINERS=20
	# bash "${stability_dir}/soak_parallel_rm.sh"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-kata) install_kata ;;
		enabling-hypervisor) enabling_hypervisor ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
