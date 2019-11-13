#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o nounset
set -o pipefail

cidir=$(dirname "$0")
arch=$("${cidir}"/kata-arch.sh -d)
source "${cidir}/lib.sh"
KATA_DEV_MODE="${KATA_DEV_MODE:-false}"
ghprbGhRepository="${ghprbGhRepository:-}"

if [ "$KATA_DEV_MODE" = true ]; then
	die "KATA_DEV_MODE set so not running the test"
fi

docker_version=$(docker version --format '{{.Server.Version}}' | cut -d '.' -f1-2)

if [ "$docker_version" != "18.06" ]; then
	die "Firecracker hypervisor only works with docker 18.06"
fi



install_fc() {
	# Get url for firecracker from runtime/versions.yaml
	firecracker_repo=$(get_version "assets.hypervisor.firecracker.url")
	[ -n "$firecracker_repo" ] || die "failed to get firecracker repo"
	firecracker_repo=${firecracker_repo/https:\/\//}

	# Get version for firecracker from runtime/versions.yaml
	firecracker_version=$(get_version "assets.hypervisor.firecracker.version")
	[ -n "$firecracker_version" ] || die "failed to get firecracker version"

	# Get firecracker repo
	go get -d "${firecracker_repo}" || true
	# Checkout to specific version
	pushd "${GOPATH}/src/${firecracker_repo}"
	git checkout "tags/${firecracker_version}"
	./tools/devtool --unattended build --release
	sudo install ${GOPATH}/src/${firecracker_repo}/build/cargo_target/${arch}-*-linux-musl/release/firecracker /usr/bin/
	sudo install ${GOPATH}/src/${firecracker_repo}/build/cargo_target/${arch}-*-linux-musl/release/jailer /usr/bin/
	popd
}

configure_fc_for_kata_and_docker() {
	echo "Configure docker"
	docker_configuration_path="/etc/docker"
	# Check if directory exists
	[ -d "$docker_configuration_path" ] || sudo mkdir "$docker_configuration_path"

	# Check if daemon.json exists
	docker_configuration_file="${docker_configuration_path}/daemon.json"

	# For Kata Containers and Firecracker a block based driver like devicemapper
	# is required
	driver="devicemapper"

	path="/usr/local/bin/kata-runtime"

	if [ -f $docker_configuration_file ]; then
		# Check devicemapper flag
		check_devicemapper=$(grep -cw '"storage-driver": "'${driver}'"' $docker_configuration_file)
		[ "$check_devicemapper" -eq 0 ] && die "${driver} is not enabled at $docker_configuration_file"
		# Check kata runtime flag
		check_kata=$(grep -cw '"path": "'${path}'"' $docker_configuration_file)
		[ "$check_kata" -eq 0 ] && die "Kata Runtime path not found at $docker_configuration_file"
	else
		cat <<-EOF | sudo tee "$docker_configuration_file"
		{
		 "runtimes": {
		  "kata-runtime": {
		   "path": "${path}"
		  }
		 },
		 "storage-driver": "${driver}"
		}
		EOF
	fi

	echo "Restart docker"
	sudo systemctl daemon-reload
	sudo systemctl restart docker

	echo "Check vsock is supported"
	if ! sudo modprobe vhost_vsock; then
		die "vsock is not supported on your host system"
	fi

}

main() {
	# Install FC only when testing changes on Kata repos.
	# If testing changes on firecracker repo, skip installation as it is
	# done in the CI jenkins job.
	if [ "${ghprbGhRepository}" != "firecracker-microvm/firecracker" ]; then
		install_fc
	fi
	configure_fc_for_kata_and_docker
}

main "$@"
