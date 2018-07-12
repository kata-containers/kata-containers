#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will perform several execs to a
# running container, the main purpose of this
# test is to stress the agent

set -e -x

cidir=$(dirname "$0")

source "${cidir}/../../metrics/lib/common.bash"

# Environment variables
IMAGE="${IMAGE:-busybox}"
CONTAINER_NAME="${CONTAINER_NAME:-test}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

# Set the runtime if not set already
RUNTIME="${RUNTIME:-kata-runtime}"

# Timeout is the duration of this test (seconds)
# We want to stress the agent for a significant
# time (approximately running for two days)
timeout=186400
start_time=$(date +%s)
end_time=$((start_time+timeout))

function setup {
	clean_env
	docker run --runtime=$RUNTIME -d --name $CONTAINER_NAME $IMAGE $PAYLOAD_ARGS
}

function exec_loop {
	docker exec $CONTAINER_NAME sh -c "echo 'hello world' > file"
	docker exec $CONTAINER_NAME sh -c "rm -rf /file"
	docker exec $CONTAINER_NAME sh -c "ls /etc/resolv.conf 2>/dev/null " | grep "/etc/resolv.conf"
	docker exec $CONTAINER_NAME sh -c "touch /tmp/execWorks"
	docker exec $CONTAINER_NAME sh -c "ls /tmp | grep execWorks"
	docker exec $CONTAINER_NAME sh -c "rm -rf /tmp/execWorks"
	docker exec $CONTAINER_NAME sh -c "ls /etc/foo" || echo "Fail expected"
	docker exec $CONTAINER_NAME sh -c "cat /tmp/one" || echo "Fail expected"
	docker exec $CONTAINER_NAME sh -c "exit 42" || echo "Fail expected"
}

function teardown {
	clean_env
}

echo "Starting stability test"
setup

echo "Running stability test"
while [[ $end_time > $(date +%s) ]]; do
	exec_loop
done

echo "Ending stability test"
teardown
