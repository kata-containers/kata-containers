#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

dir_path=$(dirname "$0")
source "${dir_path}/../../lib/common.bash"
source "${dir_path}/../../.ci/lib.sh"
source "/etc/os-release" || "source /usr/lib/os-release"
image="debian"
payload="tail -f /dev/null"
first_container_name="test1"
second_container_name="test2"
docker_configuration_path="/etc/docker"
docker_configuration_file="$docker_configuration_path/daemon.json"
file="output"

function setup() {
	clean_env
	check_processes
	# Check if directory exists
	if [ ! -d "${docker_configuration_path}" ]; then
		sudo mkdir "${docker_configuration_path}"
	fi

	# Backup existing docker configuration file
	if [ -f "${docker_configuration_file}" ]; then
		mv "${docker_configuration_file}" "${docker_configuration_file}.old"
	fi

	cat << EOF | sudo tee "${file}"
	{
	"ipv6": true,
	"fixed-cidr-v6": "2001:db8:1::/64"
	}
EOF
	cat "${file}" | sed 's/^[ \t]*//' > "${docker_configuration_file}"

	# Restart docker
	sudo systemctl restart docker
}

function test_ipv6 {
	# Run first container
	docker run -d --name "${first_container_name}" --runtime kata-runtime "${image}" sh -c "${payload}"

	# Run second container
 	docker run -d --name "${second_container_name}" --runtime kata-runtime "${image}" sh -c "${payload}"

	# Check ipv6
	check_ipv6=$(docker inspect --format='{{range .NetworkSettings.Networks}}{{.GlobalIPv6Address}}{{end}}' "${second_container_name}")
	echo "${check_ipv6}" | grep "2001:db8:1"

	# Ping containers
	docker exec "${first_container_name}" sh -c "ping -c 1 -6 ${check_ipv6}"
}

function teardown() {
	if [ -f "${docker_configuration_file}.old" ]; then
		mv "${docker_configuration_file}.old" "${docker_configuration_file}"
	else
		rm -rf "${docker_configuration_path}" "${file}"
	fi

	# Restart daemon to avoid issues in
 	# docker that says `start too quickly`
	sudo systemctl daemon-reload

	sudo systemctl restart docker

	clean_env
	check_processes
}

trap teardown EXIT

echo "Running setup"
setup

echo "Running ipv6 tests"
test_ipv6
