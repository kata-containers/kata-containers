#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

# union for 'make test'
UNION := $(shell bash -f .ci/aarch64/filter_test_aarch64.sh)

# skiped test suites for docker integration tests
SKIP := $(shell bash -f .ci/aarch64/filter_docker_aarch64.sh)
