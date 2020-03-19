#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"
DOCKERFILE_PATH="${SCRIPT_PATH}/Dockerfile"

declare -a OS_DISTRIBUTION=( \
	'ubuntu:16.04' \
	'ubuntu:18.04' \
	'fedora:30' \
	'opensuse/leap:15.1' \
	'debian:9' \
	'debian:10' \
	'centos:7' \
)

install_packages() {
	for i in "${OS_DISTRIBUTION[@]}"; do
		echo "Test OBS packages for ${OS_DISTRIBUTION}"
		run_test "${i}" "${DOCKERFILE_PATH}"
		remove_image_and_dockerfile "${i}" "${DOCKERFILE_PATH}"
	done
}

run_test() {
	local OS_DISTRIBUTION=${1:-}
	local DOCKERFILE_PATH=${2:-}
	generate_dockerfile "${OS_DISTRIBUTION}" "${DOCKERFILE_PATH}"
	build_dockerfile "${OS_DISTRIBUTION}" "${DOCKERFILE_PATH}"
}


generate_dockerfile() {
	local OS_DISTRIBUTION=${1:-}
	local DOCKERFILE_PATH=${2:-}
	DISTRIBUTION_NAME=$(echo "${OS_DISTRIBUTION}" | cut -d ':' -f1)
	case "${DISTRIBUTION_NAME}" in
		centos)
			UPDATE="yum -y update"
			DEPENDENCIES="yum install -y curl git gnupg2 lsb-release sudo"
			;;
		debian|ubuntu)
			UPDATE="apt-get -y update"
			DEPENDENCIES="apt-get --no-install-recommends install -y apt-utils ca-certificates curl git gnupg2 lsb-release sudo"
			;;
		fedora)
			UPDATE="dnf -y update"
			DEPENDENCIES="dnf install -y curl git gnupg2 sudo"
			;;
		opensuse/leap)
			UPDATE="zypper -n refresh"
			DEPENDENCIES="zypper -n install curl git gnupg sudo"
	esac

	echo "Building dockerfile for ${OS_DISTRIBUTION}"
	sed \
		-e "s|@OS_DISTRIBUTION@|${OS_DISTRIBUTION}|g" \
		-e "s|@UPDATE@|${UPDATE}|g" \
		-e "s|@DEPENDENCIES@|${DEPENDENCIES}|g" \
		"${DOCKERFILE_PATH}/Dockerfile.in" > "${DOCKERFILE_PATH}"/Dockerfile
}

build_dockerfile() {
	local OS_DISTRIBUTION=${1:-}
	local DOCKERFILE_PATH=${2:-}
	pushd "${DOCKERFILE_PATH}"
		sudo docker build \
			--build-arg http_proxy="${http_proxy}" \
			--build-arg https_proxy="${https_proxy}" \
			--tag "obs-kata-test-${OS_DISTRIBUTION}" .
	popd
}

remove_image_and_dockerfile() {
	local OS_DISTRIBUTION=${1:-}
	local DOCKERFILE_PATH=${2:-}
	echo "Removing image test-${OS_DISTRIBUTION}"
	sudo docker rmi "obs-kata-test-${OS_DISTRIBUTION}"

	echo "Removing dockerfile"
	sudo rm -f "${DOCKERFILE_PATH}/Dockerfile"
}

function main() {
	echo "Run OBS testing"
	install_packages
}

main "$@"
