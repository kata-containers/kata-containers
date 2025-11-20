#!/bin/sh
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Test script to verify kata-deploy scripts are ash-compatible
# This script itself must be ash-compatible

repo_root_dir="$(cd "$(dirname "$0")/../../.." && pwd)"
scripts_dir="${repo_root_dir}/tools/packaging/kata-deploy/scripts"

# List of scripts to test
scripts="
nfd.sh
runtime.sh
runtimeclasses.sh
snapshotters.sh
utils.sh
cri-o.sh
kata-deploy.sh
lifecycle.sh
containerd.sh
artifacts.sh
config.sh
"

# Check if ash is available
if ! command -v ash >/dev/null 2>&1; then
	echo "ERROR: ash is not available"
	exit 1
fi

errors=0

# Test 1: Verify scripts use sh shebang
echo "Test 1: Verifying scripts use sh shebang"
for script_name in ${scripts}; do
	script="${scripts_dir}/${script_name}"
	if [ ! -f "${script}" ]; then
		echo "ERROR: Script ${script} not found"
		errors=$((errors + 1))
		continue
	fi

	first_line=$(head -n 1 "${script}")
	if ! echo "${first_line}" | grep -qE '^#!/bin/sh|^#!/usr/bin/env sh'; then
		echo "ERROR: ${script} does not use sh shebang"
		echo "  Found: ${first_line}"
		errors=$((errors + 1))
	fi
done

# Test 2: Verify scripts can be parsed by ash
echo ""
echo "Test 2: Verifying scripts can be parsed by ash"
for script_name in ${scripts}; do
	script="${scripts_dir}/${script_name}"
	if [ ! -f "${script}" ]; then
		echo "ERROR: Script ${script} not found"
		errors=$((errors + 1))
		continue
	fi

	if ! ash -n "${script}" 2>&1; then
		echo "ERROR: ${script} failed ash syntax check"
		errors=$((errors + 1))
	fi
done

# Summary
echo ""
if [ ${errors} -eq 0 ]; then
	echo "All tests passed!"
	exit 0
else
	echo "Tests failed with ${errors} error(s)"
	exit 1
fi

