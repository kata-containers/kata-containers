#!/bin/bash
#Copyright (c) 2021 Intel Corporation
#
#SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")

runtime_path="/usr/local/bin/kata-runtime"
kata_config_path="/usr/share/defaults/kata-containers/configuration.toml"

results_dir="$(realpath ./)/results"

KATA_RUNTIME="${KATA_RUNTIME_CLASS:-kata}"
BAREMETAL_RUNTIME="runc"
RUNTIME_CLASS=""

FIO_SIZE="${FIO_SIZE:-500M}"
FIO_BLOCKSIZE="${FIO_BLOCKSIZE:-4K}"
VIRTIOFS_DAX_SIZE=${VIRTIOFS_DAX_SIZE:-600M}

# set the base case for virtiofsd
set_base_virtiofs_config() {
	# Running kata-qemu-virtiofs
	# Defaults for virtiofs
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_cache '"auto"'
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_cache_size ${VIRTIOFS_DAX_SIZE}
}

## helper function: get name of current bash function
fn_name() {
	echo "${FUNCNAME[1]}"
}

# directory where results are stored
get_results_dir() {
	local test_name
	local test_result_dir
	test_name="${1}"
	test_result_dir="${results_dir}/${test_name}"
	mkdir -p "${test_result_dir}"
	echo "${test_result_dir}"
}

# Collect kata env
# save kata config toml
# save output from kata-env
kata_env() {
	local suffix=${1}
	local config_path
	local kata_env_bk
	local kata_config_bk
	kata_env_bk="$(get_results_dir "${suffix}")/kata-env.toml"
	kata_config_bk="$(get_results_dir "${suffix}")/kata-config.toml"

	${runtime_path} kata-env >"${kata_env_bk}"
	config_path="$(${runtime_path} kata-env --json | jq .Runtime.Config.Path -r)"
	cp "${config_path}" "${kata_config_bk}"
}

# Collect the command used by virtiofsd
collect_qemu_virtiofs_cmd() {
	local rdir
	local test_name
	test_name="${1}"

	rdir=$(get_results_dir "${test_name}")
	# TODO
}

# Run metrics runner
run_workload() {
	local test_name
	local test_result_file
	local test_result_dir

	test_name="${1}"

	test_result_dir="$(get_results_dir "${test_name}")"
	test_result_file="${test_result_dir}/test-out.txt"

	echo "Running for kata config: ${test_name}"
	collect_qemu_virtiofs_cmd "$test_name"

	fio_runner_dir="${script_dir}/../../cmd/fiotest/"
	fio_jobs="${script_dir}/../../configs/test-config/"
	make -C "${fio_runner_dir}" build
	pwd
	set -x
	"${fio_runner_dir}fio-k8s" \
		--debug \
		--fio.size "${FIO_SIZE}" \
		--fio.block-size "${FIO_BLOCKSIZE}" \
		--container-runtime "${RUNTIME_CLASS}" \
		--test-name "${test_name}" \
		--output-dir "$(dirname ${test_result_dir})" \
		"${fio_jobs}" |
		tee \
			"${test_result_file}"
	set +x
}

pool_0_cache_auto_dax() {
	local suffix="$(fn_name)"

	set_base_virtiofs_config
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_extra_args '["--thread-pool-size=0","-o","no_posix_lock","-o","xattr"]'
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_cache '"auto"'
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_cache_size 1024
	kata_env "${suffix}"
	RUNTIME_CLASS="${KATA_RUNTIME}"
	run_workload "${suffix}"
}

pool_0_cache_auto_no_dax() {
	local suffix="$(fn_name)"

	set_base_virtiofs_config
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_extra_args '["--thread-pool-size=0","-o","no_posix_lock","-o","xattr"]'
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_cache '"auto"'
	sudo crudini --set --existing "$kata_config_path" hypervisor.qemu virtio_fs_cache_size 0

	kata_env "${suffix}"

	RUNTIME_CLASS="${KATA_RUNTIME}"
	run_workload "${suffix}"
	echo "done"
}

k8s_baremetal() {
	local suffix="$(fn_name)"

	RUNTIME_CLASS="${BAREMETAL_RUNTIME}"
	run_workload "${suffix}"
}

main() {

	mkdir -p "${results_dir}"

	k8s_baremetal
	pool_0_cache_auto_dax
	pool_0_cache_auto_no_dax
}

main $*
