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

	# Get version for firecracker from runtime/versions.yaml
	firecracker_version=$(get_version "assets.hypervisor.firecracker.version")
	[ -n "$firecracker_version" ] || die "failed to get firecracker version"

	# Download firecracker and jailer
	firecracker_binary="firecracker-${firecracker_version}-${arch}"
	curl -fsL ${firecracker_repo}/releases/download/${firecracker_version}/${firecracker_binary} -o ${firecracker_binary}
	sudo -E install -m 0755 -D ${firecracker_binary} /usr/bin/firecracker
	jailer_binary="jailer-${firecracker_version}-${arch}"
	curl -fsL ${firecracker_repo}/releases/download/${firecracker_version}/${jailer_binary} -o ${jailer_binary}
	sudo -E install -m 0755 -D ${jailer_binary} /usr/bin/jailer
}

configure_fc_for_kata_and_docker() {
	echo "Configure docker"
	# For Kata Containers and Firecracker a block based driver like devicemapper
	# is required
	storage_driver="devicemapper"
	${cidir}/../cmd/container-manager/manage_ctr_mgr.sh docker configure -r kata-runtime -s ${storage_driver} -f

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
