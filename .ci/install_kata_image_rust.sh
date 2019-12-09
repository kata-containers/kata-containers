#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

cidir=$(dirname "$0")
rust_agent_repo="github.com/kata-containers/kata-containers"
osbuilder_repo="github.com/kata-containers/osbuilder"
arch=$("${cidir}"/kata-arch.sh -d)

install_rust() {
	echo "Installing rust"
	"${cidir}/install_rust.sh"
	source $HOME/.cargo/env
}

build_rust_agent() {
	go get -d "${rust_agent_repo}" || true
	pushd "${GOPATH}/src/${rust_agent_repo}/src/agent"
	rustup target add "${arch}"-unknown-linux-musl
	echo "Building rust agent"
	make -j $(nproc)
	popd
}

build_rust_image() {
	go get -d "${osbuilder_repo}" || true
	pushd "${GOPATH}/src/${osbuilder_repo}/rootfs-builder"
	export ROOTFS_DIR="${GOPATH}/src/${osbuilder_repo}/rootfs-builder/rootfs"
	distro="ubuntu"
	sudo -E GOPATH="${GOPATH}" USE_DOCKER=true SECCOMP=no ./rootfs.sh "${distro}"
	sudo install -o root -g root -m 0550 -t "${ROOTFS_DIR}"/bin "${GOPATH}/src/${rust_agent_repo}/src/agent/target/${arch}-unknown-linux-musl/debug/kata-agent"
	sudo install -o root -g root -m 0440 ../../agent/kata-agent.service "${ROOTFS_DIR}"/usr/lib/systemd/system/
	sudo install -o root -g root -m 0440 ../../agent/kata-containers.target "${ROOTFS_DIR}"/usr/lib/systemd/system/
	popd

	pushd "${GOPATH}/src/${osbuilder_repo}"
	echo "Building rust image"
	sudo -E USE_DOCKER=1 DISTRO="${distro}" make -e image

	echo "Install rust image to /usr/share/kata-containers"
	sudo install "${GOPATH}/src/${osbuilder_repo}/kata-containers.img" "/usr/share/kata-containers/"
	popd
}

main() {
	install_rust
	build_rust_agent
	build_rust_image
}

main
