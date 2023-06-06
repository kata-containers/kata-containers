#!/bin/bash
#
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description of the test:
# This test runs the 'blogbench', and extracts the 'scores' for reads
# and writes
# Note - the scores are *not* normalised for the number of iterations run,
# they are total scores for all iterations (this is the blogbench default output)

set -e

# General env
SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../lib/common.bash"

TEST_NAME="blogbench"
IMAGE="docker.io/library/local-blogbench:latest"
DOCKERFILE="${SCRIPT_PATH}/blogbench_dockerfile/Dockerfile"

# Number of iterations for blogbench to run - note, results are not
# scaled to iterations - more iterations results in bigger results
ITERATIONS="${ITERATIONS:-30}"

# Directory to run the test on
# This is run inside of the container
TESTDIR="${TESTDIR:-/tmp}"
CMD="blogbench -i ${ITERATIONS} -d ${TESTDIR}"

function main() {
	# Check tools/commands dependencies
	cmds=("awk" "docker")

	init_env
	check_cmds "${cmds[@]}"
	check_ctr_images "$IMAGE" "$DOCKERFILE"

	metrics_json_init

	local output=$(sudo -E ${CTR_EXE} run --rm --runtime=${CTR_RUNTIME} $IMAGE test $CMD)

	# Save configuration
	metrics_json_start_array

	local frequency=$(echo "$output" | grep "Frequency" | cut -d "=" -f2 | cut -d ' ' -f2)
	local iterations=$(echo "$output" | grep -w "iterations" | cut -d ' ' -f3)
	local spawing_writers=$(echo "$output" | grep -w "writers" | cut -d ' ' -f2)
	local spawing_rewriters=$(echo "$output" | grep -w "rewriters" | cut -d ' ' -f2)
	local spawing_commenters=$(echo "$output" | grep -w "commenters" | cut -d ' ' -f2)
	local spawing_readers=$(echo "$output" | grep -w "readers" | cut -d ' ' -f2)

	local json="$(cat << EOF
	{
		"Frequency" : $frequency,
		"Iterations" : $iterations,
		"Number of spawing writers" : $spawing_writers,
		"Number of spawing rewriters" : $spawing_rewriters,
		"Number of spawing commenters" : $spawing_commenters,
		"Number of spawing readers" : $spawing_readers
	}
EOF
)"
	metrics_json_add_array_element "$json"
	metrics_json_end_array "Config"

	# Save results
	metrics_json_start_array

	local writes=$(tail -2 <<< "$output" | head -1 | awk '{print $5}')
	local reads=$(tail -1 <<< "$output" | awk '{print $6}')

	# Obtaining other Blogbench results
	local -r data=$(echo "$output" | tail -n +12 | head -n -3)
	local nb_blogs=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $1} ' | tr '\t' ',' | sed '$ s/.$//')
	local r_articles=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $2} ' | tr '\t' ',' | sed '$ s/.$//')
	local w_articles=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $3} ' | tr '\t' ',' | sed '$ s/.$//')
	local r_pictures=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $4} ' | tr '\t' ',' | sed '$ s/.$//')
	local w_pictures=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $5} ' | tr '\t' ',' | sed '$ s/.$//')
	local r_comments=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $6} ' | tr '\t' ',' | sed '$ s/.$//')
	local w_comments=$(echo "$data" | awk ' BEGIN {ORS="\t"} {print $7} ' | tr '\t' ',' | sed '$ s/.$//')

	local json="$(cat << EOF
	{
		"write": {
			"Result" : $writes,
			"Units"  : "items"
		},
		"read": {
			"Result" : $reads,
			"Units"  : "items"
		},
		"Nb blogs": {
			"Result" : "$nb_blogs"
		},
		"R articles": {
			"Result" : "$r_articles"
		},
		"W articles": {
			"Result" : "$w_articles"
		},
		"R pictures": {
			"Result" : "$r_pictures"
		},
		"W pictures": {
			"Result" : "$w_pictures"
		},
		"R comments": {
			"Result" : "$r_comments"
		},
		"W comments": {
			"Result" : "$w_comments"
		}
	}
EOF
)"

	metrics_json_add_array_element "$json"
	metrics_json_end_array "Results"
	metrics_json_save
	clean_env_ctr
}

main "$@"
