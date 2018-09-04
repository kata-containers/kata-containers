#!/bin/bash
#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

set -e

stale_process_union=( "docker-containerd-shim" )
#docker supports different storage driver, such like overlay2, aufs, etc.
docker_storage_driver=$(docker info --format='{{.Driver}}')
stale_docker_mount_point_union=( "/var/lib/docker/containers" "/var/lib/docker/${docker_storage_driver}" )
stale_docker_dir_union=( "/var/lib/docker" )
stale_kata_dir_union=( "/var/lib/vc" "/run/vc" )

lib_script="${GOPATH}/src/${tests_repo}/.ci/lib.sh"
source "${lib_script}"

metrics_lib_script="${GOPATH}/src/${tests_repo}/metrics/lib/common.bash"
source "${metrics_lib_script}"

kill_stale_process()
{
	# use function kill_processes_before_start() under $metrics_lib_script to kill stale containers or shim/proxy/hypervisor process
	kill_processes_before_start
	for stale_process in "${stale_process_union[@]}"; do
		result=$(check_processes "${stale_process}")
		if [[ $result -ne 0 ]]; then
			sudo killall -9 "${stale_process}" || true
		fi
	done <<< "${stale_process_union}"
}

delete_stale_docker_resource()
{
	local docker_status=false
	# check if docker service is running
	systemctl is-active --quiet docker
	if [ $? -eq 0 ]; then
		docker_status=true
		sudo systemctl stop docker
	fi
	# before removing stale docker dir, you should umount related resource
	for stale_docker_mount_point in "${stale_docker_mount_point_union[@]}"; do
		local mount_point_union=$(mount | grep "${stale_docker_mount_point}" | awk '{print $3}')
		if [ -n "${mount_point_union}" ]; then
			while IFS='$\n' read mount_point; do
				sudo umount "${mount_point}"
			done <<< "${mount_point_union}"
		fi
	done
	# remove stale docker dir
	for stale_docker_dir in "${stale_docker_dir_union[@]}"; do
		if [ -d "${stale_docker_dir}" ]; then
			sudo rm -rf "${stale_docker_dir}"
		fi
	done <<< "${stale_docker_dir_union}"
	[ "${docker_status}" = true ] && sudo systemctl restart docker
}

delete_stale_kata_resource()
{
	for stale_kata_dir in "${stale_kata_dir_union[@]}"; do
		if [ -d "${stale_kata_dir}" ]; then
			sudo rm -rf "${stale_kata_dir}"
		fi
	done <<< "${stale_kata_dir_union}"
}

main() {
	info "kill stale process: ${stale_process_union[@]}"
	kill_stale_process
	info "delete stale docker resource under ${stale_docker_dir_union[@]}"
	delete_stale_docker_resource
	info "delete stale kata resource under ${stale_kata_dir_union[@]}"
	delete_stale_kata_resource
}

main
