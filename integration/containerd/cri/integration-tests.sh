#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

# runc is installed in /usr/local/sbin/ add that path
export PATH="$PATH:/usr/local/sbin"

# Runtime to be used for testing
RUNTIME=${RUNTIME:-kata-runtime}
SHIMV2_TEST=${SHIMV2_TEST:-""}
FACTORY_TEST=${FACTORY_TEST:-""}

default_runtime_type="io.containerd.runtime.v1.linux"
# Type of containerd runtime to be tested
containerd_runtime_type="${default_runtime_type}"
# Runtime to be use for the test in containerd
containerd_runtime_test=${RUNTIME}
if [ -n "${SHIMV2_TEST}" ]; then
	containerd_runtime_type="io.containerd.kata.v2"
	containerd_runtime_test="io.containerd.kata.v2"
fi

readonly runc_runtime_bin=$(command -v "runc")

readonly CRITEST=${GOPATH}/bin/critest

# Flag to do tasks for CI
SNAP_CI=${SNAP_CI:-""}
CI=${CI:-""}

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
	source /etc/os-release || source /usr/lib/os-release
	ID=${ID:-""}
	if [ "$ID" == ubuntu ] &&  [ -n "${CI}" ] ;then
		# https://github.com/kata-containers/tests/issues/352
		sudo mkdir -p $(dirname "${kata_config}")
		sudo cp "${default_kata_config}" "${kata_config}"
		if [ -n "${FACTORY_TEST}" ]; then
			sudo sed -i -e 's/^#enable_template.*$/enable_template = true/g' "${kata_config}"
			echo "init vm template"
			sudo -E PATH=$PATH "$RUNTIME" factory init
		fi
	fi
	if [ -n "${CI}" ]; then
		(
		echo "Install cni config for cri-containerd test"
		cd "${GOPATH}/src/${cri_containerd_repo}"
		./hack/install/install-cni-config.sh
		)
	fi
}

ci_cleanup() {
	source /etc/os-release || source /usr/lib/os-release

	if [ -n "${FACTORY_TEST}" ]; then
		echo "destroy vm template"
		sudo -E PATH=$PATH "$RUNTIME" factory destroy
	fi

	ID=${ID:-""}
	if [ "$ID" == ubuntu ]; then
		if [ -n "${SNAP_CI}" ]; then
			# restore default configuration
			sudo cp "${default_kata_config}" "${kata_config}"
		elif [ -n "${CI}" ] ;then
			[ -f "${kata_config}" ] && sudo rm "${kata_config}"
		fi
	fi
}

create_containerd_config() {
	local runtime="$1"
	[ -n "${runtime}" ] || die "need runtime to create config"

	local runtime_type="${containerd_runtime_type}"
	if [ "${runtime}" == "runc" ]; then
		runtime_type="io.containerd.runtime.v1.linux"
	fi
	local containerd_runtime="${runtime}"
	if [ "${runtime_type}" == "${default_runtime_type}" ];then
		local containerd_runtime=$(command -v "${runtime}")
	fi
	# Remove dots.  Dots are used by toml syntax as atribute separator
	runtime="${runtime//./-}"

	cat << EOT | sudo tee "${CONTAINERD_CONFIG_FILE}"
[plugins]
  [plugins.cri]
    [plugins.cri.containerd]
	default_runtime_name = "$runtime"
      [plugins.cri.containerd.runtimes.${runtime}]
        runtime_type = "${runtime_type}"
        [plugins.cri.containerd.runtimes.${runtime}.options]
          Runtime = "${containerd_runtime}"
[plugins.linux]
       shim = "${containerd_shim_path}"
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
	info "containerd(cri): Check daemon works with runc"
	create_containerd_config "runc"

	sudo -E PATH="${PATH}:/usr/local/bin" \
		REPORT_DIR="${REPORT_DIR}" \
		FOCUS="TestImageLoad" \
		RUNTIME="" \
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

	pushd "${GOPATH}/src/${cri_containerd_repo}"

	# Make sure the right artifacts are going to be built
	make clean

	check_daemon_setup

	info "containerd(cri): testing using runtime: ${containerd_runtime_test}"

	create_containerd_config "${containerd_runtime_test}"

	info "containerd(cri): Running cri-tools"
	sudo -E PATH="${PATH}:/usr/local/bin" \
		FOCUS="runtime should support basic operations on container" \
		RUNTIME="" \
		SKIP="runtime should support execSync with timeout" \
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
			RUNTIME="" \
			CONTAINERD_CONFIG_FILE="$CONTAINERD_CONFIG_FILE" \
			make -e test-integration
	done

	popd
}

main
