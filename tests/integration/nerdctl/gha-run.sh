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
nerdctl_dir="$(dirname "$(readlink -f "$0")")"
source "${nerdctl_dir}/../../common.bash"

function install_dependencies() {
	info "Installing the dependencies for running the nerdctl tests"

	# Dependency list of projects that we can rely on the system packages
	# - wget
	#   - Used to download the nerdctl-full tarball
	# - pip
	#   - Used to install lastversion, which will be used to get the latest
	#     release of the nerdctl
        declare -a system_deps=(
		wget
		pip
	)

	sudo apt update
	sudo apt -y install "${system_deps[@]}"

	# Install lastversion from pip
	#
	# --break-system-packages is, unfortunately, needed here as it'll also
	# bring in some python3 dependencies on its own
	pip install lastversion --break-system-packages

	# As the command above will install lastversion on $HOME/.local/bin, we
	# need to add it to the PATH
	export PATH=$PATH:${HOME}/.local/bin

	# Download the nerdctl-full tarball, as it comes with all the deps
	# needed.
	nerdctl_lastest_version=$(lastversion containerd/nerdctl)
	wget https://github.com/containerd/nerdctl/releases/download/v${nerdctl_lastest_version}/nerdctl-full-${nerdctl_lastest_version}-linux-amd64.tar.gz

	# Unpack the latest nerdctl into /usr/local/
	sudo tar -xvf nerdctl-full-${nerdctl_lastest_version}-linux-amd64.tar.gz -C /usr/local/

	# Start containerd service
	sudo systemctl daemon-reload
	sudo systemctl start containerd
}

function run() {
	info "Running nerdctl smoke test tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor

	info "Running nerdctl with runc"
	sudo nerdctl run --rm --entrypoint nping instrumentisto/nmap --tcp-connect -c 2 -p 80 www.github.com

	info "Running nerdctl with Kata Containers (${KATA_HYPERVISOR})"
	sudo nerdctl run --rm --runtime io.containerd.kata.v2 --entrypoint nping instrumentisto/nmap --tcp-connect -c 2 -p 80 www.github.com
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
