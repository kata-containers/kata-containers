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
docker_dir="$(dirname "$(readlink -f "$0")")"
source "${docker_dir}/../../common.bash"
image="${image:-instrumentisto/nmap:latest}"

function install_dependencies() {
	info "Installing the dependencies needed for running the docker smoke test"

	sudo -E docker pull "${image}"
}

function run() {
	info "Running docker smoke test tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor

	info "Running docker with runc"
	sudo docker run --rm --entrypoint nping "${image}" --tcp-connect -c 2 -p 80 www.github.com

	info "Running docker with Kata Containers (${KATA_HYPERVISOR}) and --dns=8.8.8.8"
	sudo docker run --rm --dns=8.8.8.8 --dns=2001:4860:4860::8888 --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 --entrypoint nping "${image}" --tcp-connect -c 2 -p 80 www.github.com
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
