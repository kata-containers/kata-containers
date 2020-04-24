#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
vcdir="${cidir}/../src/runtime/virtcontainers/"
source "${cidir}/lib.sh"
export CI_JOB="${CI_JOB:-default}"

clone_tests_repo

if [ "${CI_JOB}" != "PODMAN" ]; then
	echo "Install virtcontainers"
	make -C "${vcdir}" && chronic sudo make -C "${vcdir}" install
fi
