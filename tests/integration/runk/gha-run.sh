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
runk_dir="$(dirname "$(readlink -f "$0")")" 
source "${runk_dir}/../../common.bash"

function install_dependencies() {
	info "Installing the dependencies needed for running the runk tests"

	# Dependency list of projects that we can rely on the system packages
	# - jq
	declare -a system_deps=(
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq

	# Dependency list of projects that we can install them
	# directly from their releases on GitHub:
	# - containerd
	#   - cri-container-cni release tarball already includes CNI plugins
	declare -a github_deps
	github_deps[0]="cri_containerd:$(get_from_kata_deps "externals.containerd.${CONTAINERD_VERSION}")"

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		install_${dep[0]} "${dep[1]}"
	done
}

function run() {
	info "Running runk tests using"

	bash -c ${runk_dir}/runk-tests.sh
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
