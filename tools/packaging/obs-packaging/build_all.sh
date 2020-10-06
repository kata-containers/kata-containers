#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
[ -z "${DEBUG}" ] || set -o xtrace

set -o errexit
set -o nounset
set -o pipefail

readonly script_name="$(basename "${BASH_SOURCE[0]}")"
readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

# shellcheck source=scripts/obs-docker.sh
source "${script_dir}/scripts/obs-pkgs.sh"

PUSH=${PUSH:-""}
LOCAL=${LOCAL:-""}

export BUILD_DISTROS=${BUILD_DISTROS:-xUbuntu_16.04}
# Packaging use this variable instead of use git user value
# On CI git user is not set
export AUTHOR="${AUTHOR:-user}"
export AUTHOR_EMAIL="${AUTHOR_EMAIL:-user@example.com}"

die() {
	local msg="${1:-}"
	local print_usage=$"${2:-}"
	if [ -n "${msg}" ]; then
		echo -e "ERROR: ${msg}\n"
	fi

	[ -n "${print_usage}" ] && usage 1
}

usage() {
	exit_code=$"${1:-0}"
	cat <<EOT
Usage:
${script_name} [-h | --help] <kata-branch> [PROJ1 PROJ2 ... ]

Generate OBS packages sources for the kata projects, based on branch
kata-branch.
${script_name} processes all the kata projects by default; alternatively you can
specify a subset of the projects as additional arguments.

Environment variables:
PUSH        When set, push the packages sources to the openSUSE build
            service.

LOCAL       When set, build the packages locally.

EOT
	exit "${exit_code}"
}

main() {
	case "${1:-}" in
		"-h"|"--help")
			usage
			;;
		-*)
			die "Invalid option: ${1:-}" "1"
			;;
		"")
			die "missing branch" "1"
			;;
		*)
			branch="${1:-}"
			;;
	esac

	shift
	local projectsList=("$@")
	[ "${#projectsList[@]}" = "0" ] && projectsList=("${OBS_PKGS_PROJECTS[@]}")

	# Make sure runtime is the last project
	projectsList=($(echo "${projectsList[@]}" | sed -E "s/(^.*)(runtime)(.*$)/\1 \3 \2/"))

	pushd "${script_dir}" >>/dev/null
	local compare_result="$(./gen_versions_txt.sh --compare ${branch})"
	[[ "$compare_result" =~ different ]] && die "$compare_result -- you need to run gen_versions_txt.sh"
	for p in "${projectsList[@]}"; do
		[ -d "$p" ] || die "$p is not a valid project directory"
		update_cmd="./update.sh"
		if [ -n "${PUSH}" ]; then
			# push to obs
			update_cmd+=" -p"
		elif [ -n "${LOCAL}" ]; then
			# local build
			update_cmd+=" -l"
		fi

		echo "======= Updating ${p} ======="
		pushd "$p" >>/dev/null
		bash -c "${update_cmd} ${branch}"
		popd >>/dev/null
		echo ""
	done
	popd >> /dev/null
}

main $@
