#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

SCRIPT_PATH=$(dirname "$0")

source "${SCRIPT_PATH}/../../lib/common.bash"

set -e

cont_name=debugconsole
tmp_file="$(mktemp)"

trap cleanup EXIT

cleanup() {
	rm -f "${tmp_file}"
	[ -f "${RUNTIME_CONFIG_PATH}.old" ] && sudo mv "${RUNTIME_CONFIG_PATH}.old" "${RUNTIME_CONFIG_PATH}"
	docker rm -f "${cont_name}" || true
}

unlock_pty() {
	pty=$1
	max_tries=10
	for _ in $(seq 1 ${max_tries}); do
		echo "" | sudo tee "${pty}"
		# looking for the prompt
		prompt="$(sudo timeout 1 cat "${pty}" | grep -E -o "[[:print:]]*" | grep '#')"
		[ -n "${prompt}" ] && return 0
	done
	return 1
}

main() {
	extract_kata_env
	if [ -z "${INITRD_PATH}" ]; then
		info "Skip: initrd path is empty"
		exit 0
	fi

	sudo cp "${RUNTIME_CONFIG_PATH}" "${RUNTIME_CONFIG_PATH}.old"

	info "Enable debug console"
	sudo crudini --set "${RUNTIME_CONFIG_PATH}" hypervisor.qemu kernel_params \"agent.debug_console\"

	info "Disable proxy debug"
	sudo crudini --set "${RUNTIME_CONFIG_PATH}" proxy.kata enable_debug false

	info "Running container"
	docker run --net=none --name=${cont_name} --runtime=${RUNTIME} -dti busybox sh

	# get console.sock path
	cont_id="$(docker inspect -f '{{.Id}}' ${cont_name})"
	console_path="$(ps -ef | grep -E "${HYPERVISOR_PATH}.*-name.*sandbox-${cont_id}.*" | grep -E -o "/run/vc/vm/[a-z0-9]+/console.sock")"

	# run socat to allocate a new pty
	info "Connect socat to ${console_path}"
	{ sudo socat -d -d unix-client:"${console_path}" pty,echo=0,raw & } > "${tmp_file}" 2>&1

	# wait for socat
	socat_success_msg="starting data transfer"
	max_tries=5
	for _ in $(seq 1 ${max_tries}); do
		grep -qi "${socat_success_msg}" "${tmp_file}" && break
		sleep 1
	done

	# get the pty path
	pty=$(grep "N PTY is" "${tmp_file}" | grep -E -o "/dev/pts/[0-9]+")
	[ -z "${pty}" ] && die "Could not get the pty path"

	# unlock pty
	info "Unlock pty: ${pty}"
	unlock_pty "${pty}" || die "Couldn't unlock pty"

	# send command
	expected_output="helloworld"
	command="echo ${expected_output}"
	echo "${command}" | sudo tee "${pty}"

	# read output and remove weird characters
	output=$(sudo timeout 1 cat "${pty}" | grep -E -o "[[:print:]]*")
	output=$(echo ${output} | grep -o "${command} ${expected_output}")

	[ -z "${output}" ] && die "Command ${command} was not executed in the VM"

	info "OK!"
}

main
