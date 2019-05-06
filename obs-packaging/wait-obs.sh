#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

script_dir=$(cd $(dirname "${BASH_SOURCE[0]}") && pwd)

run_in_docker() {
	if [ -n "${USE_DOCKER:-}" ]; then
		# shellcheck source=scripts/obs-docker.sh
		source "${script_dir}/scripts/obs-docker.sh"
		packaging_repo_dir=$(cd "${script_dir}/.." && pwd)
		docker_run "${packaging_repo_dir}/obs-packaging/wait-obs.sh"
		exit 0
	fi
}


# Check all project has finshed the build
wait_finish_building() {
	while osc pr -q | grep '(building)'; do sleep 5; done
}

# obs distro final status is 'published'
# Check all distros are published
is_published() {
	columns=$(osc pr -q -c | head -1 | column -t -s\;)
	# print to show status
	for c in ${columns}; do
		if [ "${c}" == '_' ]; then
			continue
		fi
		if ! echo "${c}" | grep 'published'; then
			echo "${c}"
			return 1
		fi
	done
	return 0
}

# Wait that all repositories are published
wait_published() {
	while ! is_published; do
		echo "Waitling for all repos are published"
	done
}

check_failed(){
	failed_query=$(osc pr -c  -s  F)
	regex=".*failed.*"
	if [[ ${failed_query} =~ ${regex} ]];then
		printf "%s" "${failed_query}" | column -t -s\;
		return 1
	fi
	return 0
}

main() {
	run_in_docker
	OBS_SUBPROJECT="${OBS_SUBPROJECT:-releases:x86_64:alpha}"
	project="home:katacontainers:${OBS_SUBPROJECT}"
	echo "Checkout: ${project}"
	osc co "$project" || true
	cd "$project" || exit 1

	echo "Wait all is build"
	wait_finish_building
	echo "OK - build finished"

	echo "Check failed"
	check_failed
	echo "OK - build did not fail"

	echo "Wait for published"
	wait_published
	echo "OK - published"
}

main $@
