#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"
export CI_JOB="${CI_JOB:-default}"

if [ "${CI_JOB}" != "PODMAN" ]; then
	run_go_test
fi
