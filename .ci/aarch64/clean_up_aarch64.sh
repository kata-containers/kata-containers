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

lib_script="${GOPATH}/src/${tests_repo}/lib/common.bash"

source "${lib_script}"

info() {
	echo -e "INFO: $*"
}

kill_stale_process()
{
	clean_env
	extract_kata_env
	stale_process_union=( "${stale_process_union[@]}" "${PROXY_PATH}" "${HYPERVISOR_PATH}" "${SHIM_PATH}" )
	for stale_process in "${stale_process_union[@]}"; do
		if pgrep -f "${stale_process}"; then
			sudo killall -9 "${stale_process}" || true
		fi
	done
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
	done
	[ "${docker_status}" = true ] && sudo systemctl restart docker
}

delete_stale_kata_resource()
{
	for stale_kata_dir in "${stale_kata_dir_union[@]}"; do
		if [ -d "${stale_kata_dir}" ]; then
			sudo rm -rf "${stale_kata_dir}"
		fi
	done
}

main() {
	info "kill stale process"
	kill_stale_process
	info "delete stale docker resource under ${stale_docker_dir_union[@]}"
	delete_stale_docker_resource
	info "delete stale kata resource under ${stale_kata_dir_union[@]}"
	delete_stale_kata_resource
}

main
