#!/bin/bash
# Copyright (c) 2018 Intel Corporation
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

HOSTINPUTDIR="${SCRIPT_PATH}/../results"
RENVFILE="${HOSTINPUTDIR}/Env.R"
HOSTOUTPUTDIR="${SCRIPT_PATH}/output"

GUESTINPUTDIR="/inputdir/"
GUESTOUTPUTDIR="/outputdir/"

setup() {
	echo "Checking subdirectories"
	check_subdir="$(cd ${HOSTINPUTDIR}; ls -dx */ > /dev/null 2>&1 | wc -l)"
	if [ $check_subdir -eq 0 ]; then
		die "Subdirectory not found at metrics/results to store JSON results"
	fi

	echo "Checking Dockerfile"
	check_dockerfiles_images "$IMAGE" "$DOCKERFILE"

	mkdir -p "$HOSTOUTPUTDIR" && true

	echo "inputdir=\"${GUESTINPUTDIR}\"" > ${RENVFILE}
	echo "outputdir=\"${GUESTOUTPUTDIR}\"" >> ${RENVFILE}

	# A bit of a hack to get an R syntax'd list of dirs to process
	# Also, need it as not host-side dir path - so short relative names
	resultdirs="$(cd ${HOSTINPUTDIR}; ls -dx */)"
	resultdirslist=$(echo ${resultdirs} | sed 's/ \+/", "/g')
	echo "resultdirs=c(" >> ${RENVFILE}
	echo "	\"${resultdirslist}\"" >> ${RENVFILE}
	echo ")" >> ${RENVFILE}
}

run() {
	docker run -ti --rm -v ${HOSTINPUTDIR}:${GUESTINPUTDIR} -v ${HOSTOUTPUTDIR}:${GUESTOUTPUTDIR} ${IMAGE}
}

setup
run
ls -la ${HOSTOUTPUTDIR}/*
