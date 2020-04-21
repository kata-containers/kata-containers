#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

# configuration file
CONFIG := .ci/aarch64/configuration_aarch64.yaml

# union for 'make test'
UNION := $(shell bash -c '.ci/filter/filter_test_union.sh $(CONFIG)')

# skiped test suites for docker integration tests
SKIP := $(shell bash -c '.ci/filter/filter_docker_test.sh $(CONFIG)')
