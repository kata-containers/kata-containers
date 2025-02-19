#!/bin/bash
#
# Copyright (c) 2018-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will perform several execs to a
# running container, the main purpose of this
# test is to stress the agent

set -x

cidir=$(dirname "$0")

source "${cidir}/../metrics/lib/common.bash"

# Environment variables
IMAGE="${IMAGE:-quay.io/prometheus/busybox:latest}"
CONTAINER_NAME="${CONTAINER_NAME:-test}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

function setup {
	clean_env_ctr
	sudo "${CTR_EXE}" image pull "${IMAGE}"
	sudo "${CTR_EXE}" run --runtime="${CTR_RUNTIME}" -d "${IMAGE}" "${CONTAINER_NAME}" sh -c "${PAYLOAD_ARGS}"
}

function exec_loop {
	cmd="sudo $CTR_EXE t exec --exec-id $(random_name) $CONTAINER_NAME sh -c"
	$cmd "echo 'hello world' > file"
	$cmd "ls /file"
	$cmd "rm -rf /file"
	$cmd "touch /tmp/execWorks"
	$cmd "ls /tmp | grep execWorks"
	$cmd "rm -rf /tmp/execWorks"
	$cmd "ls /etc/foo" || echo "Fail expected"
	$cmd "cat /tmp/one" || echo "Fail expected"
	$cmd "exit 42" || echo "Fail expected"
}

function teardown {
	echo "Ending agent stability test"
	clean_env_ctr
}
trap teardown EXIT

info "Starting stability test"
setup

info "Running agent stability test"
exec_loop
