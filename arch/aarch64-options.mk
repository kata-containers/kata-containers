#
# Copyright (c) 2018 ARM Limited
#
# SPDX-License-Identifier: Apache-2.0

# union for 'make test'
UNION := $(shell bash -f .ci/filter_test_aarch64.sh)

