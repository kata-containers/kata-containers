#!/bin/bash
#
# Copyright (c) 2026 The Kata Containers Authors
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

# shellcheck disable=SC2034
kata_tarball_dir="${2:-kata-artifacts}"
spdk_test_dir="$(dirname "$(readlink -f "$0")")"
# shellcheck source=/dev/null
source "${spdk_test_dir}/../../common.bash"

SPDK_DIR="${SPDK_DIR:-/opt/spdk}"

function install_spdk() {
	local url
	local version

	url="$(get_from_kata_deps ".externals.spdk.url")"
	version="$(get_from_kata_deps ".externals.spdk.version")"

	info "Building SPDK ${version}"
	sudo rm -rf "${SPDK_DIR}"
	sudo git clone --depth 1 --branch "${version}" "${url}" "${SPDK_DIR}"
	pushd "${SPDK_DIR}" >/dev/null
	sudo git submodule update --init --depth 1
	sudo scripts/pkgdep.sh
	# Only the vhost target and its RPC client are needed. pkgdep.sh installs
	# the Python modules of the build into this virtual environment.
	sudo bash -c "source /var/spdk/dependencies/pip/bin/activate &&
		./configure --disable-tests --disable-unit-tests --disable-examples \
			--without-crypto --without-nvme-cuse --without-uring --without-xnvme \
			--without-iscsi-initiator --without-rbd --without-daos --without-vfio-user \
			--without-idxd --without-shared &&
		make -j $(nproc)"
	popd >/dev/null
}

function install_dependencies() {
	info "Installing the dependencies needed for running the SPDK vhost-user-blk tests"

	declare -a system_deps=(
		e2fsprogs
		jq
		openssl
	)

	apt_get_update
	sudo apt-get -y install "${system_deps[@]}"

	ensure_yq

	# shellcheck disable=SC2154
	install_cri_containerd "$(get_from_kata_deps ".externals.containerd.${CONTAINERD_VERSION}")"

	install_spdk
}

function run() {
	# shellcheck disable=SC2154
	info "Running SPDK vhost-user-blk tests using ${KATA_HYPERVISOR} hypervisor"

	enabling_hypervisor
	SPDK_DIR="${SPDK_DIR}" bash -c "${spdk_test_dir}/spdk-vhost-user-blk.sh"
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-dependencies) install_dependencies ;;
		install-kata) install_kata ;;
		install-kata-tools) install_kata_tools "${2:-}" ;;
		run) run ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
