#!/bin/bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

kata_tarball_dir=${2:-kata-artifacts}
metrics_dir="$(dirname "$(readlink -f "$0")")"
source "${metrics_dir}/../common.bash"

function create_symbolic_links() {
	local link_configuration_file="/opt/kata/share/defaults/kata-containers/configuration.toml"
	local source_configuration_file="/opt/kata/share/defaults/kata-containers/configuration-${KATA_HYPERVISOR}.toml"

	if [ ${KATA_HYPERVISOR} != 'qemu' ] && [ ${KATA_HYPERVISOR} != 'clh' ]; then
		die "Failed to set the configuration.toml: '${KATA_HYPERVISOR}' is not recognized as a valid hypervisor name."
	fi

	sudo ln -sf "${source_configuration_file}" "${link_configuration_file}"
}

# Configures containerd
function overwrite_containerd_config() {
	containerd_config="/etc/containerd/config.toml"
	sudo rm "${containerd_config}"
	sudo tee "${containerd_config}" << EOF
version = 2
[plugins."io.containerd.grpc.v1.cri".containerd.runtimes.runc.options]
  SystemdCgroup = true

[plugins]
  [plugins."io.containerd.grpc.v1.cri"]
    [plugins."io.containerd.grpc.v1.cri".containerd]
      default_runtime_name = "kata"
      [plugins."io.containerd.grpc.v1.cri".containerd.runtimes]
        [plugins."io.containerd.grpc.v1.cri".containerd.runtimes.kata]
          runtime_type = "io.containerd.kata.v2"
EOF
}

function install_kata() {
	# ToDo: remove the exit once the metrics workflow is stable
	exit 0
	local kata_tarball="kata-static.tar.xz"
	declare -r katadir="/opt/kata"
	declare -r destdir="/"
	declare -r local_bin_dir="/usr/local/bin/"

	# Removing previous kata installation
	sudo rm -rf "${katadir}"

	pushd ${kata_tarball_dir}
	sudo tar -xvf "${kata_tarball}" -C "${destdir}"
	popd

	# create symbolic links to kata components
	for b in "${katadir}/bin/*" ; do
		sudo ln -sf "${b}" "${local_bin_dir}/$(basename $b)"
	done

	check_containerd_config_for_kata
	restart_containerd_service
}

function check_containerd_config_for_kata() {
	# check containerd config
	declare -r line1="default_runtime_name = \"kata\""
	declare -r line2="runtime_type = \"io.containerd.kata.v2\""
	declare -r num_lines_containerd=2
	declare -r containerd_path="/etc/containerd/config.toml"
	local count_matches=$(grep -ic  "$line1\|$line2" ${containerd_path})

	if [ $count_matches = $num_lines_containerd ]; then
		info "containerd ok"
	else
		info "overwriting containerd configuration w/ a valid one"
		overwrite_containerd_config
	fi
}

function run_test_launchtimes() {
	info "Running Launch Time test using ${KATA_HYPERVISOR} hypervisor"

	# ToDo: remove the exit once the metrics workflow is stable
	exit 0
	create_symbolic_links
	bash tests/metrics/time/launch_times.sh -i public.ecr.aws/ubuntu/ubuntu:latest -n 20
}

function run_test_memory_usage() {
	info "Running memory-usage test using ${KATA_HYPERVISOR} hypervisor"

	# ToDo: remove the exit once the metrics workflow is stable
	exit 0
	create_symbolic_links
	bash tests/metrics/density/memory_usage.sh 20 5
}

function run_test_memory_usage_inside_container() {
	info "Running memory-usage inside the container test using ${KATA_HYPERVISOR} hypervisor"

	# ToDo: remove the exit once the metrics workflow is stable
	exit 0
	create_symbolic_links
	bash tests/metrics/density/memory_usage_inside_container.sh 5
}

function main() {
	action="${1:-}"
	case "${action}" in
		install-kata) install_kata ;;
		run-test-launchtimes) run_test_launchtimes ;;
		run-test-memory-usage) run_test_memory_usage ;;
		run-test-memory-usage-inside-container) run_test_memory_usage_inside_container ;;
		*) >&2 die "Invalid argument" ;;
	esac
}

main "$@"
