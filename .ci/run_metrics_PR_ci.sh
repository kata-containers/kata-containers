#!/bin/bash
# Copyright (c) 2017-2018 Intel Corporation
# 
# SPDX-License-Identifier: Apache-2.0

# Note - no 'set -e' in this file - if one of the metrics tests fails
# then we wish to continue to try the rest.
# Finally at the end, in some situations, we explicitly exit with a
# failure code if necessary.

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_DIR}/../metrics/lib/common.bash"
RESULTS_DIR=${SCRIPT_DIR}/../metrics/results
CHECKMETRICS_DIR=${SCRIPT_DIR}/../cmd/checkmetrics

KSM_BASE="/sys/kernel/mm/ksm"
KSM_ENABLE_FILE="${KSM_BASE}/run"
KSM_PAGES_FILE="${KSM_BASE}/pages_to_scan"
KSM_SLEEP_FILE="${KSM_BASE}/sleep_millisecs"

# The settings we use for an 'aggresive' KSM setup
# Scan 1000 pages every 50ms - 20,000 pages/s
KSM_AGGRESIVE_PAGES=1000
KSM_AGGRESIVE_SLEEP=50


# Set up the initial state
init() {
	metrics_onetime_init
}

# Save the current KSM settings so we can restore them later
save_ksm_settings(){
	echo "saving KSM settings"
	ksm_stored_run=$(cat ${KSM_ENABLE_FILE})
	ksm_stored_pages=$(cat ${KSM_ENABLE_FILE})
	ksm_stored_sleep=$(cat ${KSM_ENABLE_FILE})
}

set_ksm_aggressive(){
	echo "setting KSM to aggressive mode"
	# Flip the run off/on to ensure a restart/rescan
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
	sudo bash -c "echo ${KSM_AGGRESIVE_PAGES} > ${KSM_PAGES_FILE}"
	sudo bash -c "echo ${KSM_AGGRESIVE_SLEEP} > ${KSM_SLEEP_FILE}"
	sudo bash -c "echo 1 > ${KSM_ENABLE_FILE}"
}

restore_ksm_settings(){
	echo "restoring KSM settings"
	# First turn off the run to ensure if we are then re-enabling
	# that any changes take effect
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
	sudo bash -c "echo ${ksm_stored_pages} > ${KSM_PAGES_FILE}"
	sudo bash -c "echo ${ksm_stored_sleep} > ${KSM_SLEEP_FILE}"
	sudo bash -c "echo ${ksm_stored_run} > ${KSM_ENABLE_FILE}"
}

disable_ksm(){
	echo "disabling KSM"
	sudo bash -c "echo 0 > ${KSM_ENABLE_FILE}"
}

# Execute metrics scripts
run() {
	pushd "$SCRIPT_DIR/../metrics"

	# If KSM is available on this platform, let's run any tests that are
	# affected by having KSM on/orr first, and then turn it off for the
	# rest of the tests, as KSM may introduce some extra noise in the
	# results by stealing CPU time for instance.
	if [[ -f ${KSM_ENABLE_FILE} ]]; then
		save_ksm_settings
		trap restore_ksm_settings EXIT QUIT KILL
		set_ksm_aggressive

		# Run the memory footprint test - the main test that
		# KSM affects.
		bash density/docker_memory_usage.sh 20 300 auto

		# And now ensure KSM is turned off for the rest of the tests
		disable_ksm
	fi

	# Run the time tests
	bash time/launch_times.sh -i ubuntu -n 20

	# Run the density tests - no KSM, so no need to wait for settle
	# (so set a token 5s wait)
	bash density/docker_memory_usage.sh 20 5

	# Run storage tests
	bash storage/blogbench.sh

	# Run the density test inside the container
	bash density/memory_usage_inside_container.sh

	# Run the cpu statistics test
	bash network/cpu_statistics_iperf.sh

	popd
}

# Check the results
check() {
	if [ -n "${METRICS_CI}" ]; then
		# Ensure we have the latest checkemtrics
		pushd "$CHECKMETRICS_DIR"
		make
		sudo make install
		popd

		checkmetrics --percentage --basefile /etc/checkmetrics/checkmetrics-json-$(uname -n).toml --metricsdir ${RESULTS_DIR}
		cm_result=$?
		if [ ${cm_result} != 0 ]; then
			echo "checkmetrics FAILED (${cm_result})"
			exit ${cm_result}
		fi
	fi
}

init
run
check
