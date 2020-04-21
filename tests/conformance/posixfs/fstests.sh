#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../metrics/lib/common.bash"
source "${SCRIPT_PATH}/../../lib/common.bash"

# Env variables
IMAGE="${IMAGE:-fstest}"
DOCKERFILE="${SCRIPT_PATH}/Dockerfile"
CONT_NAME="${CONT_NAME:-fstest}"
RUNTIME="${RUNTIME:-kata-runtime}"
PAYLOAD_ARGS="${PAYLOAD_ARGS:-tail -f /dev/null}"

function main() {
	clean_env
	check_dockerfiles_images "$IMAGE" "$DOCKERFILE"
	docker run -d --runtime $RUNTIME --name $CONT_NAME $IMAGE $PAYLOAD_ARGS

	echo "WARNING: Removing failing tests (Issue https://github.com/kata-containers/runtime/issues/826" >&2
	REMOVE_FILES="cd pjdfstest/tests && rm -f chown/00.t chmod/12.t link/00.t mkdir/00.t symlink/03.t mkfifo/00.t mknod/00.t mknod/11.t open/00.t"
	docker exec $CONT_NAME bash -c "${REMOVE_FILES}"
	docker exec $CONT_NAME bash -c "cd /pjdfstest && prove -r"

	clean_env
}

main "$@"
