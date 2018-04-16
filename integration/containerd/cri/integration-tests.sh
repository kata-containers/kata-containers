#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

# runc is installed in /usr/local/sbin/ add that path
export PATH="$PATH:/usr/local/sbin"

# Runtime to be used for testing
RUNTIME=${RUNTIME:-kata-runtime}
readonly runtime_bin=$(command -v "${RUNTIME}")
readonly runc_runtime_bin=$(command -v "runc")

readonly CRITEST=${GOPATH}/bin/critest

# Flag to do tasks for CI
CI=${CI:-""}

# Default CNI directory
cni_test_dir="/etc/cni/net.d"
containerd_shim_path="$(command -v containerd-shim)"
readonly cri_containerd_repo="github.com/containerd/cri"

#containerd config file
readonly tmp_dir=$(mktemp -t -d test-cri-containerd.XXXX)
export REPORT_DIR="${tmp_dir}"
readonly CONTAINERD_CONFIG_FILE="${tmp_dir}/test-containerd-config"
readonly kata_config="/etc/kata-containers/configuration.toml"
readonly default_kata_config="/usr/share/defaults/kata-containers/configuration.toml"

info() {
	echo -e "INFO: $*"
}

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

ci_config() {
	source /etc/os-release
	ID=${ID:-""}
	if [ "$ID" == ubuntu ] &&  [ -n "${CI}" ] ;then
		# https://github.com/kata-containers/tests/issues/352
		sudo mkdir -p $(dirname "${kata_config}")
		sudo cp "${default_kata_config}" "${kata_config}"
		sudo sed -i -e 's/^internetworking_model\s*=\s*".*"/internetworking_model = "bridged"/g' "/etc/kata-containers/configuration.toml"
	fi
}

ci_cleanup() {
	source /etc/os-release
	ID=${ID:-""}
	if [ "$ID" == ubuntu ] &&  [ -n "${CI}" ] ;then
		[ -f "${kata_config}" ] && sudo rm "${kata_config}"
	fi
}

create_continerd_config() {
	local runtime_config=$1
	[ -n "${runtime_config}" ] || die "need runtime to create config"

	cat > "${CONTAINERD_CONFIG_FILE}" << EOT
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
      [plugins.cri.containerd.default_runtime]
	runtime_engine = "${runtime_config}"
[plugins.linux]
	shim = "${containerd_shim_path}"
[plugins.cri.cni]
    # conf_dir is the directory in which the admin places a CNI conf.
    conf_dir = "${cni_test_dir}"
EOT
}

cleanup() {
	[ -d "$tmp_dir" ] && rm -rf "${tmp_dir}"
	ci_cleanup
}

trap cleanup EXIT

err_report() {
	echo "ERROR: containerd log :"
	echo "-------------------------------------"
	cat "${REPORT_DIR}/containerd.log"
	echo "-------------------------------------"
}

trap err_report ERR

check_daemon_setup() {
	info "containerd(cri): Check daemon works with ${runc_runtime_bin}"
	create_continerd_config "${runc_runtime_bin}"

	sudo -E PATH="${PATH}:/usr/local/bin" \
		REPORT_DIR="${REPORT_DIR}" \
		FOCUS="TestImageLoad" \
		CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE" \
		make -e test-integration
}

main() {

	info "Stop crio service"
	systemctl is-active --quiet crio && sudo systemctl stop crio

	# Configure enviroment if running in CI
	ci_config

	# make sure cri-containerd test install the proper critest version its testing
	rm -f "${CRITEST}"

	if [ -n "$CI" ]; then
		# if running on CI use a different CNI directory (cri-o and kubernetes configurations may be installed)
		cni_test_dir="/etc/cni-containerd-test"
	fi

	pushd "${GOPATH}/src/${cri_containerd_repo}"

	check_daemon_setup

	info "containerd(cri): testing using runtime: ${runtime_bin}"

	create_continerd_config "${runtime_bin}"

	info "containerd(cri): Running cri-tools"
	sudo -E PATH="${PATH}:/usr/local/bin" \
		FOCUS="runtime should support basic operations on container" \
		REPORT_DIR="${REPORT_DIR}" \
		CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE" \
		make -e test-cri

	info "containerd(cri): Running test-integration"

	passing_test=(
	TestClearContainersCreate
	TestContainerStats
	TestContainerListStatsWithIdFilter
	TestContainerListStatsWithSandboxIdFilterd
	TestContainerListStatsWithIdSandboxIdFilter
	TestDuplicateName
	TestImageLoad
	TestImageFSInfo
	TestSandboxCleanRemove
	)

	for t in "${passing_test[@]}"
	do
		sudo -E PATH="${PATH}:/usr/local/bin" \
			REPORT_DIR="${REPORT_DIR}" \
			FOCUS="${t}" \
			CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE" \
			make -e test-integration
	done

	popd
}

main
