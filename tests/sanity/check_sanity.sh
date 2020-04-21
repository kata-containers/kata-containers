#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# This will verify that before running integration and functional tests,
# processes like hypervisor, shim and proxy are not running. It will
# also check that not pod information is left and that we do not have
# runtimes running as they should be transient.

set -e

cidir=$(dirname "$0")

source "${cidir}/../lib/common.bash"

main() {
	# Check no processes are left behind
	check_processes

	# Verify that pods were not left
	check_pods

	# Verify that runtime is not running
	check_runtimes
}

main
