#!/bin/bash
#
# Copyright (c) Microsoft Corporation.
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

kata_tarball_dir="${2:-kata-artifacts}"
vfio_dir="$(dirname "$(readlink -f "$0")")"
source "${vfio_dir}/../../common.bash"

function install_dependencies() {
	info "Installing the dependencies needed for running the vfio tests"
}

function run() {
	info "Running cri-containerd tests using ${KATA_HYPERVISOR} hypervisor"
	"${vfio_dir}"/vfio_fedora_vm_wrapper.sh
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
