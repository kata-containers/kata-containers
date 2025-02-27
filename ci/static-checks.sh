#!/usr/bin/env bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/../tests/common.bash"

run_static_checks "${@:-github.com/kata-containers/kata-containers}"
