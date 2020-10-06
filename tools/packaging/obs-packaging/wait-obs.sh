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

# project information
project=""

# repo information
repo=""
repo_state=""
repo_code=""

# package information
package=""
package_code=""
package_details=""

# packages still building
packages_building=0

fail=0
return=0
continue=0

result_handler() {
	# reset project information
	project=""

	# reset repo information
	repo=""
	repo_state=""
	repo_code=""

	local IFS=' '
	for i in $1; do
		case $(echo $i | cut -d= -f1) in
			project)
				 project=$(echo $i | cut -d= -f2 | tr -d '"')
			 ;;

			 repository)
				 repo=$(echo $i | cut -d= -f2 | tr -d '"')
			 ;;

			 code)
				 repo_code=$(echo $i | cut -d= -f2 | tr -d '"')
			 ;;

			 state)
				 repo_state=$(echo $i | cut -d= -f2 | tr -d '"')
				 ;;
		esac
	done

	case "${repo_code}" in
		blocked)
			continue=1
			;;

		unresolvable)
			fail=1
			;;

		excluded)
			return=1
			;;
	esac
}

status_handler() {
	# reset package information
	package=""
	package_code=""
	package_details=""

	local IFS=' '
	for i in $1; do
		case $(echo $i | cut -d= -f1) in
			 package)
				 package=$(echo $i | cut -d= -f2 | tr -d '"')
			 ;;

			 code)
				 package_code=$(echo $i | cut -d= -f2 | tr -d '"')
			 ;;
		esac
	done

	case "${package_code}" in
		blocked)
			continue=1
			;;

		unresolvable)
			fail=1
			;;

		excluded)
			return=1
			;;
	esac
}

details_handler() {
	# reset package details
	package_details="$(echo $1 | cut -d\> -f2 | cut -d\< -f1)"

	if [ "$package_details" == "failed" ]; then
		fail=1
		osc pr
		return
	fi

	if [ "$package_details" == "broken" ]; then
		fail=1
		return
	fi

	if [ "${package_details}" != "succeeded" ] || [ "${package_code}" != "finished" ]; then
		packages_building=$((packages_building+1))
	fi
}

check_repo() {
	if [ -z "${repo}" ]; then
		return
	fi
}

dump_info() {
	echo "package: $package, code: $package_code, details: $package_details"
	echo "repository: $repo, state: $repo_state, code: $repo_code"
	echo "For more information go to https://build.opensuse.org/package/live_build_log/${project}/${package}/${repo}/$(uname -m)"
}

# Check all project has finshed the build
wait_finish_building() {
	local out
	while true; do
		sleep 30
		out=$(osc api "/build/${project}/_result")
		continue=0
		packages_building=0

		local IFS=$'\n'
		for i in ${out[*]}; do
			i="$(echo $i | sed -e 's/^[[:space:]]*//' -e 's/^<//' -e 's/>$//')"
			if echo "$i" | egrep -q "^result"; then
				result_handler "$i"
			elif echo "$i" | egrep -q "^status"; then
				status_handler "$i"
			elif echo "$i" | egrep -q "^details"; then
				details_handler "$i"
			fi

			if [ $fail -eq 1 ]; then
				echo -n "FAILED: "
				dump_info
				exit 1
			elif [ $return -eq 1 ]; then
				return
			elif [ $packages_building -gt 0 ]; then
				break
			fi
		done

		if [ $continue -eq 1 ]; then
			continue
		fi

		if [ $packages_building -gt 0 ]; then
			echo -n "BULDING: "
			dump_info
		else
			echo "FINISHED: SUCCEEDED!"
			break
		fi
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
