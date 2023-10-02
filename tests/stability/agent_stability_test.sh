#!/bin/bash
#
# Copyright (c) 2018-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will perform several execs to a
# running container, the main purpose of this
# test is to stress the agent

set -e -x

cidir=$(dirname "$0")

source "${cidir}/../metrics/lib/common.bash"

# Environment variables
IMAGE="${IMAGE:-quay.io/prometheus/busybox:latest}"
CONTAINER_NAME="${CONTAINER_NAME:-test}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"


# Timeout is the duration of this test (seconds)
# We want to stress the agent for a significant
# time (approximately running for two days)
timeout=186400
start_time=$(date +%s)
end_time=$((start_time+timeout))

function setup {
	restart_containerd_service
	sudo ctr image pull $IMAGE
	sudo ctr run --runtime=$CTR_RUNTIME -d $IMAGE $CONTAINER_NAME sh -c $PAYLOAD_ARGS
}

function exec_loop {
	cmd="sudo ctr t exec --exec-id $(random_name) $CONTAINER_NAME sh -c"
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
	echo "Ending stability test"
	clean_env_ctr
}
trap teardown EXIT

info "Starting stability test"
setup

info "Running stability test"
while [[ $end_time > $(date +%s) ]]; do
	exec_loop
done
