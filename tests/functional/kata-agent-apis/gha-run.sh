#!/usr/bin/env bash

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

kata_tarball_dir="${2:-kata-artifacts}"
kata_agent_apis_dir="$(dirname "$(readlink -f "$0")")"
source "${kata_agent_apis_dir}/../../common.bash"
source "${kata_agent_apis_dir}/../../gha-run-k8s-common.sh"

function install_dependencies() {
	info "Installing dependencies needed for testing individual agent apis using agent-ctl"

	# Dependency list of projects that we can rely on the system packages
	# - jq
	declare -a deps=(
		jq
	)

	sudo apt-get update
	sudo apt-get -y install "${deps[@]}"

	info "Installing bats"
	install_bats
}

function run() {
	bash -c ${kata_agent_apis_dir}/run-agent-api-tests.sh
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
