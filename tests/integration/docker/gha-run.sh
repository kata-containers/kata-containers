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

	info "Running docker with Kata Containers (${KATA_HYPERVISOR})"
	sudo docker run --rm --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 --entrypoint nping "${image}" --tcp-connect -c 2 -p 80 www.github.com

	# Test the network monitor
	info "Running docker with Kata Containers (${KATA_HYPERVISOR})"
	net_name="my-net"
	container_id=

	cleanup() {
		if [[ -n "$container_id" ]]; then
			info "Stop container"
			docker stop "$container_id"
			info "Delete container"
			docker rm "$container_id"
		fi
		if docker network inspect "$net_name" &>/dev/null; then
			info "Delete network"
			docker network rm "$net_name"
		fi
	}
	trap 'cleanup; trap - RETURN' RETURN

	container_id=$(sudo docker run -d --runtime "io.containerd.kata-${KATA_HYPERVISOR}.v2" busybox)

	if [ -z "$container_id" ]; then
		die "Failed to create docker container"
	fi
	info "Create a docker network '$net_name'"
	docker network create "$net_name"

	info "Connect the container to '$net_name' network"
	docker network connect "$net_name" "$container_id"
	sleep 3
	mac_address=$(docker network inspect "$net_name" | grep -A5 "$container_id" | grep '"MacAddress"' | awk -F'"' '{print $4}')
	if [ -z "$mac_address" ]; then
		die "Failed to get MacAddress"
	fi
	if docker exec -i $container_id ip a | grep "$mac_address"; then
		info "Disconnect container from '$net_name' network"
		docker network disconnect "$net_name" "$container_id"
		sleep 3
		if docker exec -i $container_id ip a | grep "$mac_address"; then
			die "Failed to disconnect from '$net_name'"
		fi
	else
		die "Failed to connect to '$net_name'"
	fi

	# cleanup is deferred with trap above
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
