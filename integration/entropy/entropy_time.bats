#!/usr/bin/env bats
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

source /etc/os-release || source /usr/lib/os-release
load "${BATS_TEST_DIRNAME}/../../lib/common.bash"
issue="https://github.com/kata-containers/tests/issues/2351"

setup() {
	[ "${ID}" == "opensuse-leap" ] && skip "test not working see: ${issue}"
	clean_env
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}

@test "measured time for /dev/random" {
	[ "${ID}" == "opensuse-leap" ] && skip "test not working see: ${issue}"
	output_file=$(mktemp)
	block_size="4b"
	expected_time="40"
	copy_number="100000"
	image="busybox"
	cmd="dd if=/dev/random of=/dev/null bs=$block_size count=$copy_number"

	docker run -i --runtime $RUNTIME $image sh -c "$cmd" 2> "$output_file"
	measured_time=$(cat $output_file | grep seconds | cut -d',' -f2 | cut -d'.' -f1)
	[ "$measured_time" -le "$expected_time" ]
}

teardown() {
	[ "${ID}" == "opensuse-leap" ] && skip "test not working see: ${issue}"
	clean_env
	rm "$output_file"
	# Check that processes are not running
	run check_processes
	echo "$output"
	[ "$status" -eq 0 ]
}
