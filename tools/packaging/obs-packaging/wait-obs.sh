#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_name="$(basename "${BASH_SOURCE[0]}")"

OBS_PROJECT=${OBS_PROJECT:-"home:katacontainers:"}
# Project to wait for
project=""

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
		docker_run "${packaging_repo_dir}/obs-packaging/wait-obs.sh" $@
		exit 0
	fi
}

# Check all project has finshed the build
wait_finish_building() {
	local out
	while true; do
		sleep 10
		out=$(osc api "/build/${project}/_result")
		if echo "${out}" | grep '<details>failed</details>'; then
			echo "Project ${project} has failed packages"
			osc pr
			exit 1
		fi
		if echo "${out}" | grep '<details>broken</details>'; then
			echo "Project ${project} has broken packages"
			exit 1
		fi
		if echo "${out}" | grep 'code="blocked"'; then
			echo "Project ${project} has blocked packages, waiting"
			continue
		fi
		if echo "${out}" | grep 'code="unresolvable"'; then
			echo "Project ${project} has unresolvable packages"
			exit 1
		fi
		if echo "${out}" | grep 'state="building"'; then
			echo "Project ${project} is still building, waiting"
			continue
		fi
		if echo "${out}" | grep 'code="excluded"'; then
			echo "Project ${project} has excluded packages left, quit waiting"
			break
		fi

		echo "No jobs with building status were found"
		echo "${out}"
		break
	done
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
			echo "waiting for : ${c}"
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

check_failed() {
	failed_query=$(osc pr -c -s F)
	if [[ ${failed_query} =~ failed ]]; then
		echo "ERROR: Build failed"
		osc pr -V -s 'F'
		exit 1
	fi
	echo "Nothing failed"
	osc pr -q -c | tail -n +2 | column -t -s\;
	return 0
}

usage() {
	msg="${1:-}"
	exit_code=$"${2:-0}"
	cat <<EOT
${msg}
Usage:
${script_name} [--options]

options:
	-h, --help: Show this help
	--no-wait-publish : no wait that OBS publish packages
EOT
	exit "${exit_code}"
}

main() {
	run_in_docker $@
	local no_wait_publish="false"
	case "${1:-}" in
	"-h" | "--help")
		usage "Help" 0
		;;
	--no-wait-publish)
		no_wait_publish="true"
		shift
		;;
	-*)
		usage "Invalid option: ${1:-}" 1
		;;
	esac
	project=${1:-}
	if [ "${project}" == "" ]; then
		OBS_SUBPROJECT="${OBS_SUBPROJECT:-}"
		project="${OBS_PROJECT}${OBS_SUBPROJECT}"
	fi
	echo "Checkout: ${project}"
	osc co "$project" || true
	cd "$project" || exit 1

	echo "Wait all is build"
	wait_finish_building
	echo "OK - build finished"

	echo "Check failed"
	check_failed
	echo "OK - build did not fail"

	if [ "${no_wait_publish}" == "true" ]; then
		echo " Requested not wait for publish"
		exit
	fi

	echo "Wait for published"
	wait_published
	echo "OK - published"
}

main $@
