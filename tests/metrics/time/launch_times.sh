#!/bin/bash
# Copyright (c) 2017-2018 Intel Corporation
# 
# SPDX-License-Identifier: Apache-2.0
#
#  Description of the test:
#  This test takes a number of time measurements through the complete
#  launch/shutdown cycle of a single container.
#  From those measurements it derives a number of time measures, such as:
#   - time to payload execution
#   - time to get to VM kernel
#   - time in VM kernel boot
#   - time to quit
#   - total time (from launch to finished)
#
# Note, the <image> used for this test must support the full 'date' command
# syntax - the date from busybox for instance *does not* support this, so
# will not work with this test.
#
# Note, this test launches a single container at a time, that quits - thus,
# this test measures times for the 'first container' only. This test does
# not look for any scalability slowdowns as the number of running containers
# increases for instance - that is handled in other tests

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

# Calculating the kernel time from dmesg stamps only really works for VM
# based runtimes - we dynamically enable it if we find we are using a known
# VM runtime
CALCULATE_KERNEL=

REQUIRED_CMDS=("bc" "awk")

# The date command format we use to ensure we capture the ns timings
# Note the no-0-padding - 0 padding the results breaks bc in some cases
DATECMD="date -u +%-s:%-N"

# This the minimum entropy level produced
# by haveged is 1000 see https://wiki.archlinux.org/index.php/Haveged
# Less than 1000 could potentially slow down cryptographic
# applications see https://www.suse.com/support/kb/doc/?id=7011351
entropy_level="1000"

check_entropy_level() {
	retries="10"
	for i in $(seq 1 "$retries"); do
		if [ $(cat /proc/sys/kernel/random/entropy_avail) -ge ${entropy_level} ]; then
			break;
		fi
		sleep 1
	done
	if [ $(cat /proc/sys/kernel/random/entropy_avail) -le ${entropy_level} ]; then
		die "Not enough entropy level to run this test"
	fi
}

# convert a 'seconds:nanoseconds' string into nanoseconds
sn_to_ns() {
	# use shell magic to strip out the 's' and 'ns' fields and print
	# them as a 0-padded ns string...
	printf "%d%09d" ${1%:*} ${1##*:}
}

# convert 'nanoseconds' (since epoch) into a 'float' seconds
ns_to_s() {
	printf "%.03f" $(bc <<< "scale=3; $1 / 1000000000")
}

run_workload() {
	start_time=$($DATECMD)

	# Check entropy level of the host
	check_entropy_level

	# Run the image and command and capture the results into an array...
	declare workload_result
	readarray -n 0 workload_result < <(docker run --cap-add SYSLOG --rm --runtime=${RUNTIME} ${NETWORK_OPTION} ${IMAGE} sh -c "$DATECMD $DMESGCMD")

	end_time=$($DATECMD)

	# Delay this calculation until after we have run - do not want
	# to measure it in the results
	start_time=$(sn_to_ns $start_time)
	end_time=$(sn_to_ns $end_time)

	# Extract the 'date' info from the first line of the log
	# This script assumes the VM clock is in sync with the host clock...
	workload_time=${workload_result[0]}
	workload_time=$(echo $workload_time | tr -d '\r')
	workload_time=$(sn_to_ns $workload_time)

	# How long did the whole launch/quit take
	total_period=$((end_time-start_time))
	# How long did it take to get to the workload
	workload_period=$((workload_time-start_time))
	# How long did it take to quit
	shutdown_period=$((end_time-workload_time))

	if [ -n "$CALCULATE_KERNEL" ]; then
		# Grab the last kernel dmesg time
		# In our case, we need to find the last real kernel line before
		# the systemd lines begin. The last:
		# 'Freeing unused kernel' line is a reasonable
		# 'last in kernel line' to look for.
		# We make a presumption here that as we are in a cold-boot VM
		# kernel, the first dmesg is at '0 seconds', so the timestamp
		# of that last line is the length of time in the kernel.
		kernel_last_line=$( (fgrep "Freeing unused kernel" <<- EOF
				${workload_result[@]}
			EOF
			) | tail -1 )

		if [ -z "$kernel_last_line" ]; then
			echo "No kernel last line"
			for l in "${workload_result[@]}"; do
				echo ">: [$l]"
			done
			die "No kernel last line"
		fi


		kernel_period=$(echo $kernel_last_line | awk '{print $2}' | tr -d "]")

		# And we can then work out how much time it took to get to the kernel
		to_kernel_period=$(printf "%0f" $(bc <<<"scale=3; $(ns_to_s $workload_period) - $kernel_period"))
	else
		kernel_period="0.0"
		to_kernel_period="0.0"
	fi

	# And store the results...
	local json="$(cat << EOF
	{
		"total": {
			"Result": $(ns_to_s $total_period),
			"Units" : "s"
		},
		"to-workload": {
			"Result": $(ns_to_s $workload_period),
			"Units" : "s"
		},
		"in-kernel": {
			"Result": $kernel_period,
			"Units" : "s"
		},
		"to-kernel": {
			"Result": $to_kernel_period,
			"Units" : "s"
		},
		"to-quit": {
			"Result": $(ns_to_s $shutdown_period),
			"Units" : "s"
		}
	}
EOF
)"
	metrics_json_add_array_element "$json"

	# If we are doing an (optional) scaling test, then we launch a permanent container
	# between each of our 'test' containers. The aim being to see if our launch times
	# are linear with the number of running containers or not
	if [ -n "$SCALING" ]; then
		docker run --runtime=${RUNTIME} -d ${IMAGE} sh -c "tail -f /dev/null"
	fi
}

init () {
	TEST_ARGS="image=${IMAGE} runtime=${RUNTIME} units=seconds"

	# We set the generic name here, but we save the different time results separately,
	# and append the actual detail to the name at the time of saving...
	TEST_NAME="boot times"

	# If we are scaling, note that in the name
	[ -n "$SCALING" ] && TEST_NAME="${TEST_NAME} scaling"

	[ -n "$NONETWORKING" ] && NETWORK_OPTION="--network none" && \
		TEST_NAME="${TEST_NAME} nonet"

	echo "Executing test: ${TEST_NAME} ${TEST_ARGS}"
	check_cmds "${REQUIRED_CMDS[@]}"

	# Only try to grab a dmesg boot time if we are pretty sure we are running a
	# Kata runtime
	local iskata=$(is_a_kata_runtime "$RUNTIME")
	if [ "$iskata" == "1" ]; then
		CALCULATE_KERNEL=1
		DMESGCMD="; dmesg"
	else
		# For non-VM runtimes, we don't use the output of dmesg, and
		# we have seen it cause some test instabilities, so do not invoke
		# it if not needed.
		DMESGCMD=""
	fi

	# Start from a fairly clean environment
	init_env
	check_images "$IMAGE"
}

help() {
	usage=$(cat << EOF
Usage: $0 [-h] [options]
   Description:
        This script takes time measurements for different
	stages of a boot/run/rm cycle
   Options:
        -d,         Disable network bringup
        -h,         Help
        -i <name>,  Image name (mandatory)
        -n <n>,     Number of containers to run (mandatory)
        -s,         Enable scaling (keep containers running)
EOF
)
	echo "$usage"
}

main() {
	local OPTIND
	while getopts "dhi:n:s" opt;do
		case ${opt} in
		d)
		    NONETWORKING=true
		    ;;
		h)
		    help
		    exit 0;
		    ;;
		i)
		    IMAGE="${OPTARG}"
		    ;;
		n)
		    TIMES="${OPTARG}"
		    ;;
		s)
		    SCALING=true
		    ;;
		?)
		    # parse failure
		    help
		    die "Failed to parse arguments"
		    ;;
		esac
	done
	shift $((OPTIND-1))

	[ -z "$IMAGE" ] && help && die "Mandatory IMAGE name not supplied"
	[ -z "$TIMES" ] && help && die "Mandatory nunmber of containers not supplied"
	# Although this is mandatory, the 'lib/common.bash' environment can set
	# it, so we may not fail if it is not set on the command line...
	[ -z "$RUNTIME" ] && help && die "Mandatory runtime argument not supplied"

	init
	metrics_json_init
	metrics_json_start_array
	for i in $(seq 1 "$TIMES"); do
		echo " run $i"
		run_workload
	done
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env
}

main "$@"
