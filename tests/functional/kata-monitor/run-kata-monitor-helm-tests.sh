#!/bin/bash
#
# Copyright (c) 2026 NVIDIA Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e
set -o pipefail

kata_monitor_dir=$(dirname "$(readlink -f "$0")")
# shellcheck source=/dev/null
source "${kata_monitor_dir}/../../common.bash"

# Setting to "yes" enables fail fast, stopping execution at the first failed test.
export BATS_TEST_FAIL_FAST="${BATS_TEST_FAIL_FAST:-no}"

if [[ -n "${KATA_MONITOR_HELM_TEST_UNION:-}" ]]; then
	KATA_MONITOR_HELM_TEST_UNION=("${KATA_MONITOR_HELM_TEST_UNION}")
else
	KATA_MONITOR_HELM_TEST_UNION=( \
		"kata-monitor.bats" \
	)
fi

run_bats_tests "${kata_monitor_dir}" KATA_MONITOR_HELM_TEST_UNION
