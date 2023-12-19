#!/bin/bash
#
# Copyright (c) 2023 Kata Contributors
#
# SPDX-License-Identifier: Apache-2.0
#
# This test will validate runk with containerd

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

tracing_dir="$(dirname "$(readlink -f "$0")")"
source "${tracing_dir}/../../common.bash"
source "${tracing_dir}/../../metrics/lib/common.bash"

RUNK_BIN_PATH="/usr/local/bin/runk"
TEST_IMAGE="docker.io/library/busybox:latest"
CONTAINER_ID="id1"
PID_FILE="${CONTAINER_ID}.pid"
WORK_DIR="$(mktemp -d --tmpdir runk.XXXXX)"

setup() {
    echo "pull container image"
    check_images ${TEST_IMAGE}
}

test_runk() {
    echo "start container with runk"
    # Bind mount ${WORK_DIR}:/tmp. Tests below will store files in this dir and check them when container is frozon.
    sudo ctr run --pid-file ${PID_FILE} -d --runc-binary ${RUNK_BIN_PATH} --mount type=bind,src=${WORK_DIR},dst=/tmp,options=rbind:rw ${TEST_IMAGE} ${CONTAINER_ID}
    read CID PID STATUS <<< $(sudo ctr t ls | grep ${CONTAINER_ID})
    [ ${PID} == $(cat ${PID_FILE}) ] || die "pid is not consistent"
    [ ${STATUS} == "RUNNING" ] || die "container status is not RUNNING"

    echo "exec process in a container"
    sudo ctr t exec --exec-id id1 ${CONTAINER_ID} sh -c "echo hello > /tmp/foo"
    [ "hello" == "$(sudo ctr t exec --exec-id id1 ${CONTAINER_ID} cat /tmp/foo)" ] || die "exec process failed"

    echo "test ps command"
    sudo ctr t exec --detach --exec-id id1 ${CONTAINER_ID} sh
    ps_out="$(sudo ctr t ps ${CONTAINER_ID})" || die "ps command failed"
    printf "ps output:\n%s\n" "${ps_out}"
    lines_no="$(printf "%s\n" "${ps_out}" | wc -l)"
    echo "ps output lines: ${lines_no}"
    # one line is the titles, and the other 2 lines are process info
    [ "3" == "${lines_no}" ] || die "unexpected ps command output"

    echo "test pause and resume"
    # The process outputs lines into /tmp/{CONTAINER_ID}, which can be read in host when it's frozon.
    sudo ctr t exec --detach --exec-id id2 ${CONTAINER_ID} sh -c "while true; do echo hello >> /tmp/${CONTAINER_ID}; sleep 0.1; done"
    # sleep for 1s to make sure the process outputs some lines
    sleep 1
    sudo ctr t pause ${CONTAINER_ID}
    [ "PAUSED" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | grep -o PAUSED)" ] || die "status is not PAUSED"
    echo "container is paused"
    local TMP_FILE="${WORK_DIR}/${CONTAINER_ID}"
    local lines1=$(cat ${TMP_FILE} | wc -l)
    # sleep for a while and check the lines are not changed.
    sleep 1
    local lines2=$(cat ${TMP_FILE} | wc -l)
    [ ${lines1} == ${lines2} ] || die "paused container is still running"
    sudo ctr t resume ${CONTAINER_ID}
    [ "RUNNING" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | grep -o RUNNING)" ] || die "status is not RUNNING"
    echo "container is resumed"
    # sleep for a while and check the lines are changed.
    sleep 1
    local lines3=$(cat ${TMP_FILE} | wc -l)
    [ ${lines2} -lt ${lines3} ] || die "resumed container is not running"

    echo "kill the container and poll until it is stopped"
    sudo ctr t kill --signal SIGKILL --all ${CONTAINER_ID}
    # poll for a while until the task receives signal and exit
    local cmd='[ "STOPPED" == "$(sudo ctr t ls | grep ${CONTAINER_ID} | awk "{print \$3}")" ]'
    waitForProcess 10 1 "${cmd}" || die "failed to kill task"

    echo "check the container is stopped"
    # there is only title line of ps command
    [ "1" == "$(sudo ctr t ps ${CONTAINER_ID} | wc -l)" ] || die "kill command failed"

    # High-level container runtimes such as containerd call the kill command with
    # --all option in order to terminate all processes inside the container
    # even if the container already is stopped. Hence, a low-level runtime
    # should allow kill --all regardless of the container state like runc.
    echo "test kill --all is allowed regardless of the container state"
    sudo ctr t kill --signal SIGKILL ${CONTAINER_ID} && die "kill should fail"
    sudo ctr t kill --signal SIGKILL --all ${CONTAINER_ID} || die "kill --all should not fail"

    echo "delete the container"
    sudo ctr t rm ${CONTAINER_ID}
    [ -z "$(sudo ctr t ls | grep ${CONTAINER_ID})" ] || die "failed to delete task"
    sudo ctr c rm ${CONTAINER_ID} || die "failed to delete container"
}

clean_up() {
    rm -f ${PID_FILE}
    rm -rf ${WORK_DIR}
}

setup
test_runk
clean_up
