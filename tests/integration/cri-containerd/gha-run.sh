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

function install_dependencies() {
	info "Installing the dependencies needed for running the cri-containerd tests"

	# Remove go if it's installed as it conflicts with another version of go
	sudo apt-get remove -y golang-* || true
	sudo rm -rf /usr/local/go

	# Remove Docker if it's installed as it conflicts with podman-docker
	sudo apt-get remove -y docker-ce-cli || true

	# Remove containerd if it's installed as it conflicts with another version of containerd
	sudo apt-get remove -y containerd containerd.io || true
	sudo rm -rf /etc/systemd/system/containerd.service

	# Dependency list of projects that we can rely on the system packages
	# - build-essential
	#   - Theoretically we only need `make`, but doesn't hurt to install
	#     the whole build-essential group
	# - jq
	# - podman-docker
	#   - one of the tests rely on docker to pull an image.
	#     we've decided to go for podman, instead, as it does *not* bring
	#     containerd as a dependency
	declare -a system_deps=(
		build-essential
		jq
		podman-docker
	)

	sudo apt-get update
	sudo apt-get -y install "${system_deps[@]}"

	# Dependency list of projects that we can install them
	# directly from their releases on GitHub:
	# - containerd
	#   - cri-container-cni release tarball already includes CNI plugins
	# - cri-tools
	declare -a github_deps
	github_deps[0]="cri_containerd:$(get_from_kata_deps ".externals.containerd.${CONTAINERD_VERSION}")"
	github_deps[1]="cri_tools:$(get_from_kata_deps ".externals.critools.latest")"
	github_deps[2]="runc:$(get_from_kata_deps ".externals.runc.latest")"
	github_deps[3]="cni_plugins:$(get_from_kata_deps ".externals.cni-plugins.version")"

	for github_dep in "${github_deps[@]}"; do
		IFS=":" read -r -a dep <<< "${github_dep}"
		"install_${dep[0]}" "${dep[1]}"
	done

	# Clone containerd as we'll need to build it in order to run the tests.
	# TODO: revert to upstream once https://github.com/containerd/containerd/pull/XXXXX
	# (fix for getRuncOptions() failing for non-runc runtimes like Kata) is merged and
	# released.
	local containerd_fork="fidencio/containerd"
	local containerd_branch="topic/fix-runc-options-type-mismatch-for-non-runc-runtimes"
	info "Cloning containerd from fork ${containerd_fork}@${containerd_branch} (temporary, pending upstream fix)"
	rm -rf containerd
	git clone -b "${containerd_branch}" "https://github.com/${containerd_fork}"

	# `make cri-integration` uses the cloned tree's `bin/containerd`, but later
	# Kata-specific tests restart the systemd service and thus use
	# `/usr/local/bin/containerd`. Install the same patched daemon there so both
	# phases exercise the same containerd build.
	info "Building and installing the patched containerd daemon for systemd restarts"
	make -C containerd bin/containerd
	sudo install -m 0755 containerd/bin/containerd /usr/local/bin/containerd
}

function run() {
	info "Running cri-containerd tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor

	bash -c "${cri_containerd_dir}/integration-tests.sh"
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
