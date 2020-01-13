#!/bin/bash
#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -x

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

cidir=$(dirname "$0")

source "${cidir}/../../lib/common.bash"

TEST_COMPATIBILITY=${TEST_COMPATIBILITY:-}

RUNTIME=${RUNTIME:-kata-runtime}

runtime_url=https://github.com/kata-containers/runtime

# directory where different versions of kata are installed
kata_versions_dir="/tmp/kata"

runtime_path=""
runtime_backup_path=""

cleanup() {
	sudo rm -rf "${kata_versions_dir}"

	if [ -n "${runtime_backup_path}" ] && [ -n "${runtime_path}" ]; then
		sudo mv "${runtime_backup_path}" "${runtime_path}"
	fi
}

trap cleanup EXIT QUIT KILL

install_kata() {
	kata_dir="$1"

	# create a new and temporal GOPATH to build kata
	tmp_gopath="$(mktemp -d)"
	kata_gopath="src/github.com/kata-containers"
	mkdir -p "${tmp_gopath}/${kata_gopath}"
	cp -a "${GOPATH}/${kata_gopath}/tests" "${tmp_gopath}/${kata_gopath}"

	# build and install kata from the new and temporal GOPATH
	pushd  "${tmp_gopath}/${kata_gopath}/tests"
	GOPATH="${tmp_gopath}" PREFIX="${kata_dir}" FORCE_BUILD_QEMU=1 ".ci/install_kata.sh"
	popd
	sudo rm -rf "${tmp_gopath}"
}

test_forward_compatibility() {
	info "Running foward compatibility test"

	runtime_path="$1"
	master_runtime_path="$2"

	# create a backup for the current runtime
	runtime_backup_path="${runtime_path}.backup"
	sudo cp "${runtime_path}" "${runtime_backup_path}"

	# run a container with the current runtime
	cont_name="forward_test"
	docker run -d --name "${cont_name}" --runtime="${RUNTIME}" busybox tail -f /dev/null

	# switch to master runtime
	sudo cp -a "${master_runtime_path}" "${runtime_path}" && sync

	# exec
	docker exec "${cont_name}" true

	# stop and remove container
	docker rm -f "${cont_name}"

	# restore runtime
	sudo cp "${runtime_backup_path}" "${runtime_path}"
}

test_backward_compatibility() {
	info "Running backward compatibility test"

	runtime_path="$1"
	master_runtime_path="$2"

	# create a backup for the current runtime
	runtime_backup_path="${runtime_path}.backup"
	sudo cp "${runtime_path}" "${runtime_backup_path}"

	# switch to master runtime
	sudo cp -a "${master_runtime_path}" "${runtime_path}" && sync

	# Now, let's run a container with master
	cont_name="backward_test"
	docker run -d --name "${cont_name}" --runtime="${RUNTIME}" busybox tail -f /dev/null

	# switch to original runtime
	sudo cp "${runtime_backup_path}" "${runtime_path}"

	# exec
	docker exec "${cont_name}" true

	# stop and remove container
	docker rm -f "${cont_name}"
}

main() {
	if [ -z "${TEST_COMPATIBILITY}" ]; then
		info "SKIP: TEST_COMPATIBILITY variable is not set"
		return 0
	fi

	runtime_path=$(get_docker_kata_path ${RUNTIME})
	if [ -z "${runtime_path}" ] || [ "${runtime_path}" = "null" ]; then
		die "${RUNTIME} has not been configured as a runtime in docker"
	fi

	# Install master version of kata.
	# current code that is being tested must be
	# compatible at least with code in master
	kata_dir="${kata_versions_dir}/master" && sudo rm -rf "${kata_dir}"
	install_kata "${kata_dir}"

	test_backward_compatibility "${runtime_path}" "${kata_dir}/bin/kata-runtime"

	# Skip test: see comments https://github.com/kata-containers/runtime/pull/2239
	#test_forward_compatibility "${runtime_path}" "${kata_dir}/bin/kata-runtime"
}

main
