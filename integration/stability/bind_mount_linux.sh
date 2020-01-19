#!/bin/bash
#
# Copyright (c) 2019 Ning Lu
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will start a container with a bind mount
# and set bind propagation, the purpose of this
# test is to check if the container stops properly

set -e

cidir=$(dirname "$0")
testname="${0##*/}"
sysname=$(uname)

if [ "${sysname}" != "Linux" ]; then
    echo "Skip ${testname} on ${sysname}"
    exit 0
fi

source "${cidir}/../../lib/common.bash"

# Environment variables
IMAGE="${IMAGE:-busybox}"
CONTAINER_NAME="${CONTAINER_NAME:-test}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"
TMP_DIR=$(mktemp -d --tmpdir ${testname}.XXX)
MOUNT_DIR="${TMP_DIR}/mount"
BIND_DST="${MOUNT_DIR}/dst"
BIND_SRC="${TMP_DIR}/src"
DOCKER_ARGS="-v ${MOUNT_DIR}:${MOUNT_DIR}:rslave"
CONTAINER_ID=

# Set the runtime if not set already
RUNTIME="${RUNTIME:-kata-runtime}"

function setup {
	clean_env
	docker run --runtime=${RUNTIME} -d ${DOCKER_ARGS} --name ${CONTAINER_NAME} ${IMAGE} ${PAYLOAD_ARGS}
	CONTAINER_ID=$(docker ps -q -f "name=${CONTAINER_NAME}")
}

function cmd_bind_mount {
	mkdir -p ${BIND_SRC}
	mkdir -p ${BIND_DST}
	mount --bind ${BIND_SRC} ${BIND_DST}
	docker rm -f ${CONTAINER_NAME}

	KATA_PROC=$(ps aux | grep ${CONTAINER_ID} | grep -v grep | tee)
}

function clean_kata_proc {
	kata_pids=$(echo -n "${KATA_PROC}" | awk '{print $2}')
	[ -n "${kata_pids}" ] && echo "${kata_pids}" | xargs kill

	kata_mount=$(mount | grep ${CONTAINER_ID} | awk '{print $3}'| sort -r)
	[ -n "${kata_mount}" ] && echo "${kata_mount}" | xargs -n1 umount

	rm -rf ${TMP_DIR}
}

function check {
	if [ -n "${KATA_PROC}" ]; then
		clean_kata_proc
		die "Left kata processes, quitting: ${KATA_PROC}"
	fi
}

function teardown {
	clean_env
	if mountpoint -q ${BIND_DST}; then
		umount ${BIND_DST}
	fi
	rm -rf ${TMP_DIR}
}

echo "Starting stability test: ${testname}"
setup

echo "Running stability test: ${testname}"
cmd_bind_mount
check

echo "Ending stability test: ${testname}"
teardown
