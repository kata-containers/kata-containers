#!/usr/bin/env bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Extract Jenkins metrics server historic data.
# Useful when re-setting the checkmetrics baseline data.

set -e

# Base dir of where we store the downloaded data. 
datadir=$(dirname "$0")/data

# How many recent builds do we evaluate
NUM_BUILDS=5

# What is the default set of repos (Jenkins jobs) we evaluate
default_repos=()
default_repos+=("kata-metrics-runtime-ubuntu-16-04-PR")
default_repos+=("kata-metrics-tests-ubuntu-16-04-PR")
repos=()

# What test results do we evaluate for each build
tests=()
test_queries=()
tests+=("boot-times")
test_queries+=(".\"boot-times\".Results | [.[] | .\"to-workload\".Result] | add / length")

tests+=("memory-footprint")
test_queries+=(".\"memory-footprint\".Results | .[] | .average.Result")

tests+=("memory-footprint-ksm")
test_queries+=(".\"memory-footprint-ksm\".Results | .[] | .average.Result")

# What is the base URL of the Jenkins server
url_base="http://jenkins.katacontainers.io/job"

# Where do we find the recent build number information
url_index="api/json"

# Where do we get the actual build results from
url_artifacts="artifact/go/src/github.com/kata-containers/tests/metrics/results"

# Gather up the results (json) files from all the defined repos for the range
# of dates?
gather_data() {
	for repo in "${repos[@]}"; do
		echo "Getting history for repo $repo"
		local outpath="${indexdir}/${repo}"
		local outname="${outpath}/index.json"
		mkdir -p "${outpath}"
		local url="${url_base}/${repo}/${url_index}"
		# First, we need the index file for the job so we can get the list of the
		# last 'n' jobs run.
		curl -L -o ${outname} $url

		builds=$(jq '.builds | .[] | .number' ${outname} | head -n ${NUM_BUILDS})

		echo "Examining builds: $builds"

		# For each build, for each test, pull down the json results file, if it
		# exists
		for build in $builds; do
			echo "Get results for build $build"
			local builddir="${resultsdir}/${repo}/${build}"
			mkdir -p ${builddir}
			local build_url="${url_base}/${repo}/${build}/${url_artifacts}/${testfilename}"
			echo "Pulling result from $build_url"
			for test in "${tests[@]}"; do
				local testfile=${builddir}/${test}.json
				local test_url="${build_url}/${test}.json"
				echo "    $test_url"
				# Can fail if the build failed to generate any results
				curl -L -o ${testfile} $test_url || true
			done
		done
	done
}

# For each test type, process all the relevant data files in the results subdir.
# *NOTE*, this does *not* take into account the number or list of build numbers we
# pulled down - it will evaluate all files it finds. If you want to only evaluate
# the data you pulled, ensure the result directory is empty (or non-existant) before
# you run the script.
process_data() {
	local count=0
	for test in "${tests[@]}"; do
		query="${test_queries[$count]}"
		echo "Processing $test"
		echo " Query '$query'"
		count=$((count+1))

		local allvalues=""
		local found=0
		local total=0
		local min=$(printf "%u" -1)
		local max=0
		files=$(find ${resultsdir} -name ${test}.json -print)
		for file in ${files}; do
			echo "  Look at file $file"
			value=$(jq "$query" $file || true)
			echo "   Result $value"
			if [ -n "$value" ]; then
				allvalues="$value $allvalues"
				found=$((found+1))
				total=$(echo $total+$value | bc)

				(( $(echo "$value > $max" | bc) )) && max=${value}
				(( $(echo "$value < $min" | bc) )) && min=${value}
			fi
		done

		mean=$(echo "scale=2; $total/$found" | bc)
		minpc=$(echo "scale=2; ($min/$mean)*100" | bc)
		maxpc=$(echo "scale=2; ($max/$mean)*100" | bc)
		pc_95=$(echo "scale=2; $mean*0.95" | bc)
		pc_105=$(echo "scale=2; $mean*1.05" | bc)

		echo "allvalues are [$allvalues]"
		echo "${test}: mean $mean, 95% mean ${pc_95}, 105% mean ${pc_105}"
		echo "         min $min ($minpc% of mean), max $max ($maxpc% of mean)"
	done
}

help() {
	usage=$(cat << EOF
Usage: $0 [-h] [options]
   Description:
        Gather statistics from recent Jenkins CI metrics builds. The resulting
        data is useful for configuring the metrics slave checkmetrics baselines.

        To change which metrics tests are evaluated, edit the values in this
        script directly. Default tests evaluated are:
          "${tests[@]}"

   Options:
        -d <path>,   Directory to store downloaded data (default: ${datadir})
        -h,          Print this help
        -n <n>,      Fetch last 'n' build data from Jenkins server (default: ${NUM_BUILDS})
                     Note: The statistics calculations include *all* data files in the
                     directory: ${resultsdir}. If previous data exists, it will be counted.
        -r <remote>, Which Jenkins build jobs to gather data from.
           (default: "${default_repos[@]}")
EOF
)
	echo "$usage"
}

main() {
	local OPTIND
	while getopts "d:hn:r:" opt;do
		case ${opt} in
		d)
		    datadir="${OPTARG}"
		    ;;
		h)
		    help
		    exit 0;
		    ;;
		n)
		    NUM_BUILDS="${OPTARG}"
		    ;;
		r)
		    repos+=("${OPTARG}")
		    ;;
		?)
		    # parse failure
		    help
		    echo "Failed to parse arguments" >&2
		    exit -1
		    ;;
		esac
	done
	shift $((OPTIND-1))

	[ -z "${repos[@]}" ] && repos=(${default_repos[@]})

	resultsdir="${datadir}/results"
	indexdir="${datadir}/indexes"

	gather_data
	process_data
}

main "$@"
