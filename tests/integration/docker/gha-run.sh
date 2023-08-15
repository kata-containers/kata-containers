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
	container_id=$(sudo docker run -d --runtime io.containerd.kata-${KATA_HYPERVISOR}.v2 busybox)
	if [ -z "$container_id" ]; then
		die "Failed to create docker"
	fi
	info "Create a docker network named 'my-net'"
	docker network create my-net

	info "Connect the container to the 'my-net' network"
	docker network connect my-net $container_id
	sleep 3
	mac_address=$(docker network inspect my-net | grep -A5 $container_id | grep '"MacAddress"' | awk -F'"' '{print $4}')
	if [ -z "$mac_address" ]; then
		die "Failed to get MacAddress"
	fi
	if docker exec -i $container_id ip a | grep "$mac_address"; then
		info "Disconnect the container from the 'my-net' network"
		docker network disconnect my-net $container_id
		sleep 3
		if docker exec -i $container_id ip a | grep "$mac_address"; then
			die "Failed to disconnect to "my-net""
		fi
	else
		die "Failed to connect to "my-net""
	fi

	info "Stop the container"
	docker stop $container_id

	info "Delete the container"
	sudo docker rm $container_id

	info "Delete the network "
	sudo docker network rm my-net
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
