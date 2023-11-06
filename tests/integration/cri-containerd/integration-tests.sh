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
source "${SCRIPT_PATH}/lib.sh"

# runc is installed in /usr/local/sbin/ add that path
export PATH="$PATH:/usr/local/sbin"

# golang is installed in /usr/local/go/bin/ add that path
export PATH="$PATH:/usr/local/go/bin"

ARCH=$(uname -m)

containerd_runtime_type="io.containerd.kata-${KATA_HYPERVISOR}.v2"

containerd_shim_path="$(command -v containerd-shim)"

#containerd config file
readonly tmp_dir=$(mktemp -t -d test-cri-containerd.XXXX)
export REPORT_DIR="${tmp_dir}"
readonly CONTAINERD_CONFIG_FILE="${tmp_dir}/test-containerd-config"
readonly CONTAINERD_CONFIG_FILE_TEMP="${CONTAINERD_CONFIG_FILE}.temp"
readonly default_containerd_config_backup="$CONTAINERD_CONFIG_FILE.backup"

function cleanup() {
	ci_cleanup
	[ -d "$tmp_dir" ] && rm -rf "${tmp_dir}"
}

trap cleanup EXIT

function check_daemon_setup() {
	info "containerd(cri): Check daemon works with runc"
	create_containerd_config "runc"

	# containerd cri-integration will modify the passed in config file. Let's
	# give it a temp one.
	cp $CONTAINERD_CONFIG_FILE $CONTAINERD_CONFIG_FILE_TEMP
	# in some distros(AlibabaCloud), there is no btrfs-devel package available,
	# so pass GO_BUILDTAGS="no_btrfs" to make to not use btrfs.
	sudo -E PATH="${PATH}:/usr/local/bin" \
		REPORT_DIR="${REPORT_DIR}" \
		FOCUS="TestImageLoad" \
		RUNTIME="" \
		CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE_TEMP" \
		make GO_BUILDTAGS="no_btrfs" -e cri-integration
}

function main() {

	info "Stop crio service"
	systemctl is-active --quiet crio && sudo systemctl stop crio

	info "Stop containerd service"
	systemctl is-active --quiet containerd && stop_containerd

	# Configure enviroment if running in CI
	ci_config

	pushd "containerd"

	# Make sure the right artifacts are going to be built
	make clean

	check_daemon_setup

	info "containerd(cri): testing using runtime: ${containerd_runtime_type}"

	create_containerd_config "kata-${KATA_HYPERVISOR}"

	info "containerd(cri): Running cri-integration"


	passing_test="TestContainerStats|TestContainerRestart|TestContainerListStatsWithIdFilter|TestContainerListStatsWithIdSandboxIdFilter|TestDuplicateName|TestImageLoad|TestImageFSInfo|TestSandboxCleanRemove"

	if [[ "${KATA_HYPERVISOR}" == "cloud-hypervisor" || \
		"${KATA_HYPERVISOR}" == "qemu" ]]; then
		issue="https://github.com/kata-containers/tests/issues/2318"
		info "${KATA_HYPERVISOR} fails with TestContainerListStatsWithSandboxIdFilter }"
		info "see ${issue}"
	else
		passing_test="${passing_test}|TestContainerListStatsWithSandboxIdFilter"
	fi

	# in some distros(AlibabaCloud), there is no btrfs-devel package available,
	# so pass GO_BUILDTAGS="no_btrfs" to make to not use btrfs.
	# containerd cri-integration will modify the passed in config file. Let's
	# give it a temp one.
	cp $CONTAINERD_CONFIG_FILE $CONTAINERD_CONFIG_FILE_TEMP
	sudo -E PATH="${PATH}:/usr/local/bin" \
		REPORT_DIR="${REPORT_DIR}" \
		FOCUS="^(${passing_test})$" \
		RUNTIME="" \
		CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE_TEMP" \
		make GO_BUILDTAGS="no_btrfs" -e cri-integration
}

main
