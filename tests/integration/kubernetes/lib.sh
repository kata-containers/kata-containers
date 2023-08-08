#!/bin/bash
# Copyright (c) 2021, 2022 IBM Corporation
# Copyright (c) 2022, 2023 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#
# This provides generic functions to use in the tests.
#

# Wait until the pod is not 'Ready'. Fail if it hits the timeout.
#
# Parameters:
#	$1 - the pod name
#	$2 - wait time in seconds. Defaults to 90. (optional)
#
wait_pod_to_be_ready() {
	local pod_name="$1"
	local wait_time="${2:-90}"

	kubectl wait --timeout="${wait_time}s" --for=condition=ready "pods/$pod_name"
}