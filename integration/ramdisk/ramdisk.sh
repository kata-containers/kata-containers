#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

cidir=$(dirname "$0")

source "${cidir}/../../lib/common.bash"

set -e

# Environment variables
IMAGE="${IMAGE:-busybox}"
CONTAINER_NAME="${CONTAINER_NAME:-test}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-sleep 30}"

# Set the runtime if not set already
RUNTIME="${RUNTIME:-kata-runtime}"

# File to save ramdisk results
TMP_FILE=$(mktemp ramdisk.XXXXXXXXXX)

remove_tmp_file() {
	rm -rf $TMP_FILE
}

trap remove_tmp_file EXIT

setup() {
	clean_env

	# Stop docker
	sudo -E systemctl stop docker
}

test_ramdisk() {
	extract_kata_env

	# Grab the time (filter journalctl)
	grab_time=$(date +"%H:%M:%S")

	# Enable ramdisk with dockerd
	(sudo -E DOCKER_RAMDISK=true dockerd -D --add-runtime ${RUNTIME}=${RUNTIME_PATH} 2>/dev/null) &

	# Run container
	docker run --rm -d --name=${CONTAINER_NAME} --runtime=${RUNTIME} ${IMAGE} sh -c ${PAYLOAD_ARGS}

	# Extract the logs of kata-runtime
	sudo journalctl -t ${RUNTIME} --since ${grab_time} > ${TMP_FILE}

	# Verify that --no-pivot flag is at the log
	check_pivot=$(grep -E -o "\-\-\no\-\pivot" ${TMP_FILE} | wc -l)
	if [ $check_pivot -eq 0 ]; then
		echo >&2 "ERROR: --no-pivot flag was not found"
		exit 1
	fi
}

teardown() {
	dockerd_id=$(pgrep -f dockerd)
	for i in "${dockerd_id[@]}"; do
		sudo kill -9 $i
	done

	# Start docker
	sudo -E systemctl start docker

	clean_env

	# Check that processes are not running
	check_processes
}

echo "Running setup"
setup

echo "Running ramdisk test"
test_ramdisk

echo "Running teardown"
teardown
