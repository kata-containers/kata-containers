#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir=$(dirname $(readlink -f "$0"))
docker_image="cloud-hypervisor-builder"

DOCKER_CLI="docker"

if ! command -v docker && command -v podman; then
	DOCKER_CLI="podman"
fi

sudo "${DOCKER_CLI}" build -t "${docker_image}" "${script_dir}"

if test -t 1; then
	USE_TTY="-ti"
else
	USE_TTY=""
	echo "INFO: not tty build"
fi

sudo "${DOCKER_CLI}" run \
	--rm \
	-v "$(pwd):/$(pwd)" \
	-w "$(pwd)" \
	--env "CARGO_HOME=$(pwd)" \
	${USE_TTY} \
	"${docker_image}" \
	cargo build --release
