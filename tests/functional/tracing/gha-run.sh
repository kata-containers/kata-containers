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
tracing_dir="$(dirname "$(readlink -f "$0")")" 
source "${tracing_dir}/../../common.bash"

function install_dependencies() {
	info "Installing the dependencies needed for running the tracing tests"

	# Dependency list of projects that we can rely on the system packages
	# - crudini
	# - jq
	# - socat
	# - tmux
	declare -a system_deps=(
		crudini
		jq
		socat
		tmux
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	# Install docker according to the docker's website documentation
	install_docker
}

function run() {
	info "Running tracing tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor
	bash -c ${tracing_dir}/test-agent-shutdown.sh
	bash -c ${tracing_dir}/tracing-test.sh
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
