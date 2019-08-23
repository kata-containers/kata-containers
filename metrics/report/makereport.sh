#!/bin/bash
# Copyright (c) 2018-2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Take the data found in subdirectories of the metrics 'results' directory,
# and turn them into a PDF report. Use a Dockerfile containing all the tooling
# and scripts we need to do that.

set -e

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

IMAGE="${IMAGE:-metrics-report}"
DOCKERFILE="${SCRIPT_PATH}/report_dockerfile/Dockerfile"

HOST_INPUT_DIR="${SCRIPT_PATH}/../results"
R_ENV_FILE="${HOST_INPUT_DIR}/Env.R"
HOST_OUTPUT_DIR="${SCRIPT_PATH}/output"

GUEST_INPUT_DIR="/inputdir/"
GUEST_OUTPUT_DIR="/outputdir/"

# If in debugging mode, we also map in the scripts dir so you can
# dynamically edit and re-load them at the R prompt
HOST_SCRIPT_DIR="${SCRIPT_PATH}/report_dockerfile"
GUEST_SCRIPT_DIR="/scripts/"

setup() {
	echo "Checking subdirectories"
	check_subdir="$(ls -dx ${HOST_INPUT_DIR}/*/ 2> /dev/null | wc -l)"
	if [ $check_subdir -eq 0 ]; then
		die "No subdirs in [${HOST_INPUT_DIR}] to read results from."
	fi

	echo "Checking Dockerfile"
	check_dockerfiles_images "$IMAGE" "$DOCKERFILE"

	mkdir -p "$HOST_OUTPUT_DIR" && true

	echo "inputdir=\"${GUEST_INPUT_DIR}\"" > ${R_ENV_FILE}
	echo "outputdir=\"${GUEST_OUTPUT_DIR}\"" >> ${R_ENV_FILE}

	# A bit of a hack to get an R syntax'd list of dirs to process
	# Also, need it as not host-side dir path - so short relative names
	resultdirs="$(cd ${HOST_INPUT_DIR}; ls -dx */)"
	resultdirslist=$(echo ${resultdirs} | sed 's/ \+/", "/g')
	echo "resultdirs=c(" >> ${R_ENV_FILE}
	echo "	\"${resultdirslist}\"" >> ${R_ENV_FILE}
	echo ")" >> ${R_ENV_FILE}
}

run() {
	docker run -ti --rm -v ${HOST_INPUT_DIR}:${GUEST_INPUT_DIR} -v ${HOST_OUTPUT_DIR}:${GUEST_OUTPUT_DIR} ${extra_volumes} ${IMAGE} ${extra_command}
	ls -la ${HOST_OUTPUT_DIR}/*
}

help() {
	usage=$(cat << EOF
Usage: $0 [-h] [options]
   Description:
        This script generates a metrics report document
        from the results directory one level up in the
        directory tree (../results).
   Options:
        -d,         Run in debug (interactive) mode
        -h,         Print this help
EOF
)
	echo "$usage"
}

main() {

	local OPTIND
	while getopts "d" opt;do
		case ${opt} in
		d)
			# In debug mode, run a shell instead of the default report generation
			extra_command="bash"
			extra_volumes="-v ${HOST_SCRIPT_DIR}:${GUEST_SCRIPT_DIR}"
			;;
		?)
		    # parse failure
		    help
		    die "Failed to parse arguments"
		    ;;
		esac
	done
	shift $((OPTIND-1))

	setup
	run
}

main "$@"
