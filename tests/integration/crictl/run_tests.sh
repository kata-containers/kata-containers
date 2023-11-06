#!/bin/bash
#
# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[[ "${DEBUG}" != "" ]] && set -o xtrace
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../common.bash"
source "${SCRIPT_PATH}/../cri-containerd/lib.sh"

# golang is installed in /usr/local/go/bin/ add that path
export PATH="$PATH:/usr/local/go/bin"

export ARCH=$(uname -m)

containerd_runtime_type="io.containerd.kata-${KATA_HYPERVISOR}.v2"

containerd_shim_path="$(command -v containerd-shim)"

#containerd config file
export tmp_dir=$(mktemp -t -d test-cri-containerd.XXXX)
export REPORT_DIR="${tmp_dir}"
export CONTAINERD_CONFIG_FILE="${tmp_dir}/test-containerd-config"
export CONTAINERD_CONFIG_FILE_TEMP="${CONTAINERD_CONFIG_FILE}.temp"
export default_containerd_config_backup="$CONTAINERD_CONFIG_FILE.backup"

TESTS_UNION=(
	generic.bats \
	container_update.bats\
	container_swap.bats \
	)

function cleanup() {
	ci_cleanup
	[ -d "$tmp_dir" ] && rm -rf "${tmp_dir}"
}

trap cleanup EXIT

function err_report() {
	echo "::group::ERROR - containerd logs"
	echo "-------------------------------------"
	sudo journalctl -xe -t containerd
	echo "-------------------------------------"
	echo "::endgroup::"

	echo "::group::ERROR - Kata Containers logs"
	echo "-------------------------------------"
	sudo journalctl -xe -t kata
	echo "-------------------------------------"
	echo "::endgroup::"
}

function main() {

	info "Stop crio service"
	systemctl is-active --quiet crio && sudo systemctl stop crio

	info "Stop containerd service"
	systemctl is-active --quiet containerd && stop_containerd

	# Configure enviroment if running in CI
	ci_config

	pushd "containerd"
	make GO_BUILDTAGS="no_btrfs"
	sudo -E PATH="${PATH}:/usr/local/bin" \
		make install
	popd

	create_containerd_config "kata-${KATA_HYPERVISOR}"

	# trap error for print containerd and kata-containers log
	trap err_report ERR

	pushd "$SCRIPT_PATH"
	bats ${TESTS_UNION[@]}
	popd
}

main
