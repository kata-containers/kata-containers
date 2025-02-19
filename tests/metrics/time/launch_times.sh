#!/bin/bash
# Copyright (c) 2017-2023 Intel Corporation
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

# set the total number of decimal digits after the decimal point
# for representing the calculations results
CALC_SCALE=4

# The date command format we use to ensure we capture the ns timings
# Note the no-0-padding - 0 padding the results breaks bc in some cases
DATECMD="date -u +%-s:%-N"

# The modern Linux RNG is extremely fast at generating entropy on demand
# and does not need to have as large a store of entropy anymore as the value
# of 256 was found to work well with common cryptographic algorithms
entropy_level="256"

# Grabs the number of iterations performed
num_iters=0

# sets to this max number of repetitons for failed runs
MAX_REPETITIONS=3

# The individual results are stored in an array
declare -a total_result_ds
declare -a to_workload_ds
declare -a in_kernel_ds
declare -a to_kernel_ds
declare -a to_quit_ds
# data_is_valid value 1 represent not valid
# data_is_valid value 0 represent is valid
data_is_valid=0

function check_entropy_level() {
	retries="10"
	for i in $(seq 1 "${retries}"); do
		if [ $(cat "/proc/sys/kernel/random/entropy_avail") -ge "${entropy_level}" ]; then
			break;
		fi
		sleep 1
	done
	if [ $(cat "/proc/sys/kernel/random/entropy_avail") -lt "${entropy_level}" ]; then
		die "Not enough entropy level to run this test"
	fi
}

# convert a 'seconds:nanoseconds' string into nanoseconds
function sn_to_ns() {
	# !!: Remove 0's from beginning otherwise the number will be converted to octal
	s=$(echo "${1%:*}" | sed 's/^0*//g')
	ns=$(echo "${1##*:}" | sed 's/^0*//g')
	# use shell magic to strip out the 's' and 'ns' fields and print
	# them as a 0-padded ns string...
	printf "%d%09d" "${s}" "${ns}"
}

# convert 'nanoseconds' (since epoch) into a 'float' seconds
function ns_to_s() {
	printf "%.0${CALC_SCALE}f" $(bc <<< "scale=$CALC_SCALE; $1 / 1000000000")
}

function run_workload() {
	# L_CALC_SCALE is set to accounting a significant
	# number of decimal digits after the decimal points
	# for 'bc' performing math in kernel period estimation
	L_CALC_SCALE=13
	local CONTAINER_NAME="kata_launch_times_$(( $RANDOM % 1000 + 1))"
	start_time=$(eval "${DATECMD}")

	# Check entropy level of the host
	check_entropy_level

	# Run the image and command and capture the results into an array...
	declare workload_result
	readarray -n 0 workload_result < <(sudo -E "${CTR_EXE}" run --rm --runtime "${CTR_RUNTIME}" "${IMAGE}" "${CONTAINER_NAME}" bash -c "${DATECMD} ${DMESGCMD}")
	end_time=$(eval "${DATECMD}")

	# Delay this calculation until after we have run - do not want
	# to measure it in the results
	start_time=$(sn_to_ns "${start_time}")
	end_time=$(sn_to_ns "${end_time}")

	# Extract the 'date' info from the first line of the log
	# This script assumes the VM clock is in sync with the host clock...
	workload_time="${workload_result[0]}"
	workload_time=$(echo "${workload_time}" | tr -d '\r')
	workload_time=$(sn_to_ns "${workload_time}")

	# How long did the whole launch/quit take
	total_period=$((end_time-start_time))
	# How long did it take to get to the workload
	workload_period=$((workload_time-start_time))
	# How long did it take to quit
	shutdown_period=$((end_time-workload_time))

	if [ -n "${CALCULATE_KERNEL}" ]; then
		# Grab the last kernel dmesg time
		# In our case, we need to find the last real kernel line before
		# the systemd lines begin. The last:
		# 'Freeing unused kernel' line is a reasonable
		# 'last in kernel line' to look for.
		# We make a presumption here that as we are in a cold-boot VM
		# kernel, the first dmesg is at '0 seconds', so the timestamp
		# of that last line is the length of time in the kernel.
		kernel_last_line=$( (grep -F "Freeing unused kernel" <<- EOF
				${workload_result[@]}
			EOF
			) | tail -1 )

		if [ -z "${kernel_last_line}" ]; then
			echo "No kernel last line"
			for l in "${workload_result[@]}"; do
				echo ">: [$l]"
			done
			die "No kernel last line"
		fi

		kernel_period=$(echo "${kernel_last_line}" | awk '{print $2}' | tr -d "]")

		# And we can then work out how much time it took to get to the kernel
		to_kernel_period=$(printf "%f" $(bc <<<"scale=$L_CALC_SCALE; $(ns_to_s $workload_period) - $kernel_period"))
	else
		kernel_period="0.0"
		to_kernel_period="0.0"
	fi

	total_result="$(ns_to_s ${total_period})"
	to_workload="$(ns_to_s ${workload_period})"
	in_kernel="${kernel_period}"
	to_kernel="${to_kernel_period}"
	to_quit=$(ns_to_s "${shutdown_period}")

	tr_is_neg=$(echo "${total_result}"'<='0.0 | bc -l)
	tw_is_neg=$(echo "${to_workload}"'<='0.0 | bc -l)
	ik_is_neg=$(echo "${in_kernel}"'<='0.0 | bc -l)
	tk_is_neg=$(echo "${to_kernel}"'<='0.0 | bc -l)
	tq_is_neg=$(echo "${to_quit}"'<='0.0 | bc -l)

	data_is_valid=0
	if [ "${tr_is_neg}" -eq 1 ] || [ "${tw_is_neg}" -eq 1 ] || [ "${ik_is_neg}" -eq 1 ] || [ "${tk_is_neg}" -eq 1 ] || [ "${tq_is_neg}" -eq 1 ]; then
		data_is_valid=1
	else
		# Insert results individually
		total_result_ds+=("${total_result}")
		to_workload_ds+=("${to_workload}")
		in_kernel_ds+=("${in_kernel}")
		to_kernel_ds+=("${to_kernel}")
		to_quit_ds+=("${to_quit}")
	fi

	((num_iters+=1))

	# If we are doing an (optional) scaling test, then we launch a permanent container
	# between each of our 'test' containers. The aim being to see if our launch times
	# are linear with the number of running containers or not
	if [ -n "${SCALING}" ]; then
		sudo -E "${CTR_EXE}" run --runtime="${CTR_RUNTIME}" -d "${IMAGE}" test bash -c "tail -f /dev/null"
	fi
}

# Writes a JSON  with the measurements
# results per execution
function write_individual_results() {
	for i in "${!total_result_ds[@]}"; do
		local json="$(cat << EOF
	{
		"total": {
			"Result": ${total_result_ds[i]},
			"Units": "s"
		},
		"to-workload": {
			"Result": ${to_workload_ds[i]},
			"Units": "s"
		},
		"in-kernel": {
			"Result": ${in_kernel_ds[i]},
			"Units": "s"
		},
		"to-kernel": {
			"Result": ${to_kernel_ds[i]},
			"Units": "s"
		},
		"to-quit": {
			"Result": ${to_quit_ds[i]},
			"Units": "s"
		}
	}
EOF
)"
		metrics_json_add_array_element "$json"
	done
}

function init() {
	TEST_ARGS="image=${IMAGE} runtime=${CTR_RUNTIME} units=seconds"

	# We set the generic name here, but we save the different time results separately,
	# and append the actual detail to the name at the time of saving...
	TEST_NAME="boot times"

	# If we are scaling, note that in the name
	[ -n "$SCALING" ] && TEST_NAME="${TEST_NAME} scaling"

	info "Executing test: ${TEST_NAME} ${TEST_ARGS}"
	check_cmds "${REQUIRED_CMDS[@]}"

	# For non-VM runtimes, we don't use the output of dmesg, and
	# we have seen it cause some test instabilities, so do not invo>
	# it if not needed.
	if [ "${CTR_RUNTIME}" == "io.containerd.runc.v2" ]; then
		DMESGCMD=""
	else
		CALCULATE_KERNEL=1
		DMESGCMD="; dmesg"
	fi

	# Start from a fairly clean environment
	init_env
	check_images "${IMAGE}"
}

# Computes the average of the data
function calc_avg_array() {
	data=("$@")
	avg=0
	LSCALE=6
	size="${#data[@]}"

	[ -z "${data}" ] && die "List of results was not passed to the calc_avg_array() function when trying to calculate the average result."
	[ "${size}" -eq 0 ] && die "Division by zero: The number of items is 0 when trying to calculate the average result."

	sum=$(IFS='+'; echo "scale=4; ${data[*]}" | bc)
	avg=$(echo "scale=$LSCALE; ${sum} / ${size}" | bc)
	printf "%.0${CALC_SCALE}f" "${avg}"
}


# Computes the standard deviation of the data
function calc_sd_array() {
	data=("$@")
	sum_sqr_n=0
	size="${#data[@]}"

	# LSCALE is the scale used for calculations in the middle
	# CALC_SCALE is the scale used for the result
	LSCALE=13
	CALC_SCALE=6

	[ -z "${data}" ] && die "List results was not passed to the calc_sd_result() function when trying to calculate the standard deviation result."
	[ "${size}" -eq 0 ] && die "Division by zero: The number of items is 0 when trying to calculate the standard deviation result."


	# [1] sum data
	sum_data=$(IFS='+'; echo "scale=$LSCALE; ${data[*]}" | bc)

	# [2] square the sum of data
	pow_2_sum_data=$(echo "scale=$LSCALE; $sum_data ^ 2" | bc)

	# [3] divide the square of data by the num of items
	div_sqr_n=$(echo "scale=$LSCALE; $pow_2_sum_data / $size" | bc)

	# [4] Sum of the sqr of each item
	for i in "${data[@]}"; do
		sqr_n=$(echo "scale=$LSCALE; $i ^ 2" | bc)
		sum_sqr_n=$(echo "scale=$LSCALE; $sqr_n + $sum_sqr_n" | bc)
	done

	# substract [4] from [3]
	subs=$(echo "scale=$LSCALE; $sum_sqr_n - $div_sqr_n" | bc)

	# get variance
	var=$(echo "scale=$LSCALE; $subs / $size" | bc)

	# get standard deviation
	sd=$(echo "scale=$LSCALE; sqrt($var)" | bc)

	# if sd is zero, limit the decimal scale to 1 digit
	sd_is_zero=$(echo "${sd}"'=='0.0 | bc -l)
	[ "${sd_is_zero}" -eq 1 ] && CALC_SCALE=1

	printf "%.0${CALC_SCALE}f" "${sd}"
}

# Computes the Coefficient of variation.
# The result is given as percentage.
function calc_cov_array() {
	sd=$1
	mean=$2

	# LSCALE used for consider more decimals digits than usual in cov estimation.
	# CALC_SCALE is the scale used to return the result.
	LSCALE=13
	CALC_SCALE=6

	mean_is_zero=$(echo "${mean}"'=='0.0 | bc -l)

	[ -z "${sd}" ] && die "Standard deviation was not passed to the calc_cov_array() function when trying to calculate the CoV result."
	[ -z "${mean}" ] && die "Mean was not passed to the calc_cov_array() function when trying to calculate the CoV result."
	[ "${mean_is_zero}" -eq 1 ] && die "Division by zero: Mean value passed is 0 when trying to get CoV result."

	cov=$(echo "scale=$LSCALE; $sd / $mean" | bc)
	cov=$(echo "scale=$LSCALE; $cov * 100" | bc)

	# if cov is zero, limit the decimal scale to 1 digit
	cov_is_zero=$(echo "${cov}"'=='0.0 | bc -l)
	[ "${cov_is_zero}" -eq 1 ] && CALC_SCALE=1

	printf "%.0${CALC_SCALE}f" "${cov}"
}

# Writes a JSON with the statistics results
# for each launch time metric
function write_stats_results() {
	size="${#total_result_ds[@]}"
	avg_total_result=$(calc_avg_array "${total_result_ds[@]}")
	avg_to_workload=$(calc_avg_array "${to_workload_ds[@]}")
	avg_in_kernel=$(calc_avg_array "${in_kernel_ds[@]}")
	avg_to_kernel=$(calc_avg_array "${to_kernel_ds[@]}")
	avg_to_quit=$(calc_avg_array "${to_quit_ds[@]}")

	sd_total_result=$(calc_sd_array "${total_result_ds[@]}")
	sd_to_workload=$(calc_sd_array "${to_workload_ds[@]}")
	sd_in_kernel=$(calc_sd_array "${in_kernel_ds[@]}")
	sd_to_kernel=$(calc_sd_array "${to_kernel_ds[@]}")
	sd_to_quit=$(calc_sd_array "${to_quit_ds[@]}")

	cov_total_result=$(calc_cov_array "${sd_total_result}" "${avg_total_result}")
	cov_to_workload=$(calc_cov_array "${sd_to_workload}" "${avg_to_workload}")
	cov_in_kernel=$(calc_cov_array "${sd_in_kernel}" "${avg_in_kernel}")
	cov_to_kernel=$(calc_cov_array "${sd_to_kernel}" "${avg_to_kernel}")
	cov_to_quit=$(calc_cov_array "${sd_to_quit}" "${avg_to_quit}")

	local json="$(cat << EOF
	{
	"size": $size,
	"total": {
		"avg": $avg_total_result,
		"sd": $sd_total_result,
		"cov": $cov_total_result
	},
	"to-workload": {
		"avg": $avg_to_workload,
		"sd": $sd_to_workload,
		"cov": $cov_to_workload
	},
	"in-kernel": {
		"avg": $avg_in_kernel,
		"sd": $sd_in_kernel,
		"cov": $cov_in_kernel
	},
	"to-kernel_avg": {
		"avg": $avg_to_kernel,
		"sd": $sd_to_kernel,
		"cov": $cov_to_kernel
	},
	"to-quit": {
		"avg": $avg_to_quit,
		"sd": $sd_to_quit,
		"cov": $cov_to_quit
	}
	}
EOF
)"
	metrics_json_add_array_element "$json"
}

function help() {
	usage=$(cat << EOF
Usage: $0 [-h] [options]
   Description:
        This script takes time measurements for different
	stages of a boot/run/rm cycle
   Options:
        -h,         Help
        -i <name>,  Image name (mandatory)
        -n <n>,     Number of containers to run (mandatory)
        -s,         Enable scaling (keep containers running)
EOF
)
	echo "$usage"
}

function main() {
	local OPTIND
	while getopts "dhi:n:s" opt;do
		case ${opt} in
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

	[ -z "${IMAGE}" ] && help && die "Mandatory IMAGE name not supplied"
	[ -z "${TIMES}" ] && help && die "Mandatory nunmber of containers not supplied"
	# Although this is mandatory, the 'lib/common.bash' environment can set
	# it, so we may not fail if it is not set on the command line...
	[ -z "${RUNTIME}" ] && help && die "Mandatory runtime argument not supplied"

	init
	j=0
	max_reps="${MAX_REPETITIONS}"

	while [ "${j}" -lt "${TIMES}" ]; do

		info " run ${num_iters}"
		run_workload

		if [ "${data_is_valid}" -eq 0 ]; then
			j=$(( j + 1 ))
			# if valid result then reset 'max_reps' to initial value
			max_reps="${MAX_REPETITIONS}"
			continue
		fi

		info "Skipping run due to invalid result"
		((max_reps-=1))

		if [ "${max_reps}" -lt 0 ]; then
			die "Max. num of repetitions reached for run: $j"
		fi
	done

	metrics_json_init
	metrics_json_start_array
	write_stats_results
	metrics_json_end_array "Statistics"
	metrics_json_start_array
	write_individual_results
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env_ctr
}

main "$@"
