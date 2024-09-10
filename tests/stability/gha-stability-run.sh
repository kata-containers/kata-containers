#!/bin/bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

stability_dir="$(dirname "$(readlink -f "$0")")"
source "${stability_dir}/../common.bash"
source "${stability_dir}/../metrics/lib/common.bash"

function run_tests() {
	info "Running scability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/kubernetes_stability.sh"

	info "Running soak stability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/kubernetes_soak_test.sh"

	info "Running stressng stability test using ${KATA_HYPERVISOR} hypervisor"
	bash "${stability_dir}/kubernetes_stressng.sh"
}

function main() {
	action="${1:-}"
	case "${action}" in
		run-tests) run_tests ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"

