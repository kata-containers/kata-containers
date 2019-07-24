#!/bin/bash
#
# Copyright (c) 2019 HyperHQ Inc.
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will kill a running container's
# hypervisor, and see how we react to cleanup.

set -e

cidir=$(dirname "$0")

source "${cidir}/../../metrics/lib/common.bash"

# Environment variables
IMAGE="${IMAGE:-busybox}"
CONTAINER_NAME="${CONTAINER_NAME:-test}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

# Set the runtime if not set already
RUNTIME="${RUNTIME:-kata-runtime}"

KATA_HYPERVISOR="${KATA_HYPERVISOR:-qemu}"

if [ "$KATA_HYPERVISOR" == "firecracker" ]; then
	issue="https://github.com/kata-containers/tests/issues/1849"
	echo "Skip hypervisor stability kill test, see: $issue"
	exit
fi

setup()  {
	clean_env
	sudo docker run --runtime=$RUNTIME -d --name $CONTAINER_NAME $IMAGE $PAYLOAD_ARGS
	num=$(ps aux | grep ${HYPERVISOR_PATH} | grep -v grep | wc -l)
	[ ${num} -eq 1 ] || die "hypervisor count:${num} expected:1"
}

kill_hypervisor()  {
	pid=$(ps aux | grep ${HYPERVISOR_PATH} | grep -v grep | awk '{print $2}')
	[ -n ${pid} ] || die "failed to find hypervisor pid"
	sudo kill -KILL ${pid} || die "failed to kill hypervisor (pid ${pid})"
	num=$(ps aux | grep ${HYPERVISOR_PATH} | grep -v grep | wc -l)
	[ ${num} -eq 0 ] || die "hypervisor count:${num} expected:0"
	sudo docker rm -f $CONTAINER_NAME
	[ $? -eq 0 ] || die "failed to force removing container $CONTAINER_NAME"
}

teardown()  {
	echo "Ending hypervisor stability test"
	clean_env
}

trap teardown EXIT

echo "Starting hypervisor stability test"
setup

echo "Running hypervisor stability test"
kill_hypervisor
