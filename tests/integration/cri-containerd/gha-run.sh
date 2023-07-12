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
cri_containerd_dir="$(dirname "$(readlink -f "$0")")" 
source "${cri_containerd_dir}/../../common.bash"

function run() {
	info "Running cri-containerd tests using ${KATA_HYPERVISOR} hypervisor"

	create_symbolic_links ${KATA_HYPERVISOR}
	return 0
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-kata) install_kata ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
