#!/usr/bin/env bats
#
# Copyright (c) 2023,2024 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will validate runk with containerd

load "${BATS_TEST_DIRNAME}/../../common.bash"
load "${BATS_TEST_DIRNAME}/../../metrics/lib/common.bash"

setup_file() {
	export RUNK_BIN_PATH="/usr/local/bin/runk"
	export TEST_IMAGE="quay.io/prometheus/busybox:latest"
	export CONTAINER_ID="id1"
	export PID_FILE="${CONTAINER_ID}.pid"
	export WORK_DIR="${BATS_FILE_TMPDIR}"

	echo "pull container image"
	check_images ${TEST_IMAGE}
}

setup() {
	# Bind mount ${WORK_DIR}:/tmp. Tests below will store files in this dir and check them when container is frozon.
	sudo ctr run --pid-file ${PID_FILE} -d \
		--mount type=bind,src=${WORK_DIR},dst=/tmp,options=rbind:rw \
		--runc-binary ${RUNK_BIN_PATH} \
		${TEST_IMAGE} \
		${CONTAINER_ID}
	read CID PID STATUS <<< $(sudo ctr t ls | grep ${CONTAINER_ID})
	# Check the pid is consistent
	[ "${PID}" == "$(cat "${PID_FILE}")" ]
	# Check the container status is RUNNING
	[ "${STATUS}" == "RUNNING" ]
}

teardown() {
	echo "delete the container"
	if sudo ctr t list -q | grep -q "${CONTAINER_ID}"; then
		stop_container
	fi
	sudo ctr c rm "${CONTAINER_ID}"
	sudo rm -f "${PID_FILE}"
}

stop_container() {
	local cmd
	sudo ctr t kill --signal SIGKILL --all "${CONTAINER_ID}"
	# poll for a while until the task receives signal and exit
	cmd='[ "STOPPED" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | awk "{print \$3}")" ]'
	waitForProcess 10 1 "${cmd}"

	echo "check the container is stopped"
	# there is only title line of ps command
	[ "1" == "$(sudo ctr t ps ${CONTAINER_ID} | wc -l)" ]
}

@test "start container with runk" {
}

@test "exec process in a container" {
	sudo ctr t exec --exec-id id1 "${CONTAINER_ID}" sh -c "echo hello > /tmp/foo"
	# Check exec succeeded
	[ "hello" == "$(sudo ctr t exec --exec-id id1 "${CONTAINER_ID}" cat /tmp/foo)" ]
}

@test "run ps command" {
	sudo ctr t exec --detach --exec-id id1 "${CONTAINER_ID}" sh

	return_code=$?
	echo "ctr t exec sh return: ${return_code}"

	# Give some time for the sh process to start within the container.
	sleep 5
	ps_out="$(sudo ctr t ps ${CONTAINER_ID})" || die "ps command failed"
	printf "ps output:\n%s\n" "${ps_out}"
	lines_no="$(printf "%s\n" "${ps_out}" | wc -l)"
	echo "ps output lines: ${lines_no}"
	# one line is the titles, and the other 2 lines are process info
	[ "3" == "${lines_no}" ]
}

@test "pause and resume the container" {
	# The process outputs lines into /tmp/{CONTAINER_ID}, which can be read in host when it's frozon.
	sudo ctr t exec --detach --exec-id id2 ${CONTAINER_ID} \
		sh -c "while true; do echo hello >> /tmp/${CONTAINER_ID}; sleep 0.1; done"
	# sleep for 1s to make sure the process outputs some lines
	sleep 1
	sudo ctr t pause "${CONTAINER_ID}"
	# Check the status is PAUSED
	[ "PAUSED" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | grep -o PAUSED)" ]
	echo "container is paused"
	local TMP_FILE="${WORK_DIR}/${CONTAINER_ID}"
	local lines1=$(cat ${TMP_FILE} | wc -l)
	# sleep for a while and check the lines are not changed.
	sleep 1
	local lines2=$(cat ${TMP_FILE} | wc -l)
	# Check the paused container is not running the process (paused indeed)
	[ ${lines1} == ${lines2} ]
	sudo ctr t resume ${CONTAINER_ID}
	# Check the resumed container has status of RUNNING
	[ "RUNNING" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | grep -o RUNNING)" ]
	echo "container is resumed"
	# sleep for a while and check the lines are changed.
	sleep 1
	local lines3=$(cat ${TMP_FILE} | wc -l)
	# Check the process is running again
	[ ${lines2} -lt ${lines3} ]
}

@test "kill the container and poll until it is stopped" {
	stop_container
}

@test "kill --all is allowed regardless of the container state" {
	# High-level container runtimes such as containerd call the kill command with
	# --all option in order to terminate all processes inside the container
	# even if the container already is stopped. Hence, a low-level runtime
	# should allow kill --all regardless of the container state like runc.
	echo "test kill --all is allowed regardless of the container state"
	# Check kill should fail because the container is paused
	stop_container
	run sudo ctr t kill --signal SIGKILL ${CONTAINER_ID}
	[ $status -eq 1 ]
	# Check  kill --all should not fail
	sudo ctr t kill --signal SIGKILL --all "${CONTAINER_ID}"
}
