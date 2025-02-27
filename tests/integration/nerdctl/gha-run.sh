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
	pip install lastversion

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

	# Create the default containerd configuration
	sudo mkdir -p /etc/containerd
	containerd config default > sudo tee /etc/containerd/config.toml
	sudo systemctl restart containerd
}

function collect_artifacts() {
	if [ -z "${start_time:-}" ]; then
		warn "tests start time is not defined. Cannot gather journal information"
		return
	fi

	local artifacts_dir="/tmp/artifacts"
	if [ -d "${artifacts_dir}" ]; then
		rm -rf "${artifacts_dir}"
	fi
	mkdir -p "${artifacts_dir}"
	info "Collecting artifacts using ${KATA_HYPERVISOR} hypervisor"
	local journalctl_log_filename="journalctl-$RANDOM.log"
	local journalctl_log_path="${artifacts_dir}/${journalctl_log_filename}"
	sudo journalctl --since="$start_time" > "${journalctl_log_path}"
}

function run() {
	info "Running nerdctl smoke test tests using RunC"

	info "Running nerdctl with runc"
	sudo nerdctl run --rm --entrypoint nping instrumentisto/nmap --tcp-connect -c 2 -p 80 www.github.com

	local parent_interface="eth0"
	# The following creates an ipvlan network with eth0 on host as parent. The test assumes
	# that an interface called eth0 exists on the host.
	local ipvlan_net_name="ipvlan10"
	info "Creating ipvlan network with eth0 interface on host as parent"
	sudo nerdctl network create ${ipvlan_net_name} --driver ipvlan --subnet=10.5.74.0/24 -o parent=${parent_interface}

	# The following creates an ipvlan network with eth0 on host as parent.
	local macvlan_net_name="macvlan20"
	info "Creating macvlan network with eth0 interface on host as parent"
	sudo nerdctl network create ${macvlan_net_name} --driver ipvlan --subnet=10.8.0.0/24 -o parent=${parent_interface}

	# Create two bridge networks for testing multiple networks with Kata
	local net1="foo"
	local net2="bar"
	sudo nerdctl network create ${net1}
	sudo nerdctl network create ${net2}

	enabling_hypervisor

	if [ -n "${GITHUB_ENV:-}" ]; then
		start_time=$(date '+%Y-%m-%d %H:%M:%S')
		export start_time
		echo "start_time=${start_time}" >> "$GITHUB_ENV"
	fi

	info "Running nerdctl smoke test tests using ${KATA_HYPERVISOR} hypervisor"

	info "Running nerdctl with Kata Containers (${KATA_HYPERVISOR})"
	sudo nerdctl run --rm --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 --entrypoint nping instrumentisto/nmap --tcp-connect -c 2 -p 80 www.github.com

	info "Running nerdctl with Kata Containers (${KATA_HYPERVISOR}) and multiple bridge nwtorks"
	sudo nerdctl run --rm --net ${net1} --net ${net2} --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 alpine ip a

	info "Running nerdctl with Kata Containers (${KATA_HYPERVISOR}) and ipvlan network"
	sudo nerdctl run  --rm --net ${ipvlan_net_name}  --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 alpine ip a | grep "eth0"

	info "Running nerdctl with Kata Containers (${KATA_HYPERVISOR}) and macvlan network"
	sudo nerdctl run  --rm --net ${macvlan_net_name}  --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 alpine ip a | grep "eth0"

	info "Removing networks"
	sudo nerdctl network rm ${macvlan_net_name} ${ipvlan_net_name}
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		install-kata) install_kata ;;
		run) run ;;
		collect-artifacts) collect_artifacts ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
