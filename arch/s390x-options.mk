#
# Copyright (c) 2019 IBM
#
# SPDX-License-Identifier: Apache-2.0

# union for 'make test'
UNION := $(shell bash -f .ci/s390x/filter_test_s390x.sh)

# skiped test suites for docker integration tests
SKIP := $(shell bash -f .ci/s390x/filter_docker_s390x.sh)
