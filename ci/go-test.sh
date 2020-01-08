#
# Copyright (c) 2020 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

export TEST_DOCKER=true
run_go_test
