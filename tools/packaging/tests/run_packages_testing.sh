#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
http_proxy="${http_proxy:-}"
https_proxy="${https_proxy:-}"
DOCKERFILE_PATH="${SCRIPT_PATH}/Dockerfile"
OS_DISTRIBUTION="${OS_DISTRIBUTION:-fedora:31}"

install_packages() {
	echo "Test distribution packages for ${OS_DISTRIBUTION}"
	run_test
	remove_image_and_dockerfile
}

run_test() {
	generate_dockerfile
	build_dockerfile
}

generate_dockerfile() {
	UPDATE="dnf -y update"
	DEPENDENCIES="dnf install -y curl git make sudo golang dnf-plugin-config-manager"

	echo "Building dockerfile for ${OS_DISTRIBUTION}"
	sed \
		-e "s|@OS_DISTRIBUTION@|${OS_DISTRIBUTION}|g" \
		-e "s|@DEPENDENCIES@|${DEPENDENCIES}|g" \
		"${DOCKERFILE_PATH}/FedoraDockerfile.in" > "${DOCKERFILE_PATH}"/Dockerfile
}

build_dockerfile() {
	pushd "${DOCKERFILE_PATH}"
		sudo docker build \
			--build-arg http_proxy="${http_proxy}" \
			--build-arg https_proxy="${https_proxy}" \
			--tag "packaging-kata-test-${OS_DISTRIBUTION}" .
	popd
}

remove_image_and_dockerfile() {
	echo "Removing image test-${OS_DISTRIBUTION}"
	sudo docker rmi "packaging-kata-test-${OS_DISTRIBUTION}"

	echo "Removing dockerfile"
	sudo rm -f "${DOCKERFILE_PATH}/Dockerfile"
}

function main() {
	echo "Run packaging testing"
	install_packages
}

main "$@"
