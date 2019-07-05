#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will make sure that guest time is synced with the host

set -e

load "${BATS_TEST_DIRNAME}/../../lib/common.bash"

setup() {
	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}

@test "Verify guest time is synced with the host" {
	CONTAINER_NAME="test"
	IMAGE="busybox"
	PAYLOAD="tail -f /dev/null"

	docker run -d --name "${CONTAINER_NAME}" --runtime "${RUNTIME}" "${IMAGE}" sh -c "${PAYLOAD}"

	# Get host and guest time
	GUEST_CMD="docker exec "${CONTAINER_NAME}" date +'%H:%M:%S'"
	HOST_CMD="date +'%H:%M:%S'"
	OUTPUT=$((echo "${GUEST_CMD}"; echo "${HOST_CMD}") | parallel)
	echo $OUTPUT

	GUEST_TIME=$(echo $OUTPUT | cut -d ' ' -f1)
	HOST_TIME=$(echo $OUTPUT | cut -d ' ' -f2)

	TIME_DIFF=$(( $(date -d "$GUEST_TIME" "+%s")-$(date -d "$HOST_TIME" "+%s") ))
	echo "$TIME_DIFF"
	[ "$TIME_DIFF" -le 1 ]
}

teardown() {
	docker rm -f "${CONTAINER_NAME}"

	clean_env

	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
