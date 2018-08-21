#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Run a set of the metrics tests to gather data to be used with the report
# generator. The general ideal is to have the tests configured to generate
# useful, meaninful and repeatable (stable, with minimised variance) results.
# If the tests have to be run more or longer to achieve that, then generally
# that is fine - this test is not intended to be quick, it is intended to
# be repeatable.

# Note - no 'set -e' in this file - if one of the metrics tests fails
# then we wish to continue to try the rest.
# Finally at the end, in some situations, we explicitly exit with a
# failure code if necessary.

SCRIPT_DIR=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_DIR}/../lib/common.bash"
RESULTS_DIR=${SCRIPT_DIR}/../results

# Set up the initial state
init() {
	metrics_onetime_init
}

# Execute metrics scripts
run() {
	pushd "$SCRIPT_DIR/.."

	# If KSM is available on this platform, let's run any tests that are
	# affected by having KSM on/off first, and then turn it off for the
	# rest of the tests, as KSM may introduce some extra noise in the
	# results by stealing CPU time for instance.
	if [[ -f ${KSM_ENABLE_FILE} ]]; then
		save_ksm_settings
		trap restore_ksm_settings EXIT QUIT KILL
		set_ksm_aggressive

		# Run the memory footprint test - the main test that
		# KSM affects. Run for 20 containers (that gives us a fair
		# view of how memory gets shared across containers), and
		# a default timeout of 300s - if KSM has not settled down
		# by then, just take the measurement.
		# 'auto' mode should detect when KSM has settled automatically.
		bash density/docker_memory_usage.sh 20 300 auto

		# Grab scaling system level footprint data for different sized
		# container workloads - with KSM enabled.

		# busybox - small container
		export PAYLOAD_SLEEP="1"
		export PAYLOAD="busybox"
		export PAYLOAD_ARGS="tail -f /dev/null"
		export PAYLOAD_RUNTIME_ARGS=" -m 2G"
		bash density/footprint_data.sh

		# mysql - medium sized container
		export PAYLOAD_SLEEP="10"
		export PAYLOAD="mysql"
		PAYLOAD_ARGS=" --innodb_use_native_aio=0 --disable-log-bin"
		PAYLOAD_RUNTIME_ARGS=" -m 4G -e MYSQL_ALLOW_EMPTY_PASSWORD=1"
		bash density/footprint_data.sh

		# elasticsearch - large container
		export PAYLOAD_SLEEP="10"
		export PAYLOAD="elasticsearch"
		PAYLOAD_ARGS=" "
		PAYLOAD_RUNTIME_ARGS=" -m 8G"
		bash density/footprint_data.sh

		# And now ensure KSM is turned off for the rest of the tests
		disable_ksm
	fi

	# Run the time tests - take time measures for an ubuntu image, over
	# 100 'first and only container' launches.
	# NOTE - whichever container you test here must support a full 'date'
	# command - busybox based containers (including Alpine) will not work.
	bash time/launch_times.sh -i ubuntu -n 100

	# Run the density tests - no KSM, so no need to wait for settle
	# (so set a token 5s wait). Take the measure across 20 containers.
	bash density/docker_memory_usage.sh 20 5

	popd
}

finish() {
	echo "Now please create a suitably descriptively named subdirectory in"
	echo "$RESULTS_DIR and copy the .json results files into it before running"
	echo "this script again."
}

init
run
finish

