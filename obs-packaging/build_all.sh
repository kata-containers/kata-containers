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
#Note:Lets update qemu and the kernel first, they take longer to build.
#Note: runtime is build at the end to get the version from all its dependencies.
projects=(
	qemu-lite
	qemu-vanilla
	kernel
	kata-containers-image
	proxy
	shim
	ksm-throttler
	runtime
)

OSCRC="${HOME}/.oscrc"
PUSH=${PUSH:-""}
LOCAL=${LOCAL:-""}
PUSH_TO_OBS=""

export BUILD_DISTROS=${BUILD_DISTROS:-xUbuntu_16.04}
# Packaging use this variable instead of use git user value
# On CI git user is not set
export AUTHOR="${AUTHOR:-user}"
export AUTHOR_EMAIL="${AUTHOR_EMAIL:-user@example.com}"

OBS_API="https://api.opensuse.org"

usage() {
	msg="${1:-}"
	exit_code=$"${2:-0}"
	cat <<EOT
${msg}
Usage:
${script_name} <kata-branch>
EOT
	exit "${exit_code}"
}

main() {
	local branch="${1:-}"
	[ -n "${branch}" ] || usage "missing branch" "1"
	if [ -n "${OBS_USER:-}" ] && [ -n "${OBS_PASS:-}" ] && [ ! -e "${OSCRC:-}" ]; then
		echo "Creating  ${OSCRC} with user $OBS_USER"
		cat <<eom >"${OSCRC}"
[general]
apiurl = ${OBS_API}
[${OBS_API}]
user = ${OBS_USER}
pass = ${OBS_PASS}
eom
	fi

	pushd "${script_dir}"
	for p in "${projects[@]}"; do
		pushd "$p" >>/dev/null
		update_cmd="./update.sh"
		if [ -n "${PUSH}" ]; then
			# push to obs
			update_cmd+=" -p"
		elif [ -n "${LOCAL}" ]; then
			# local build
			update_cmd+=" -l"
		fi

		echo "update ${p}"
		bash -c "${update_cmd} ${branch}"
		popd >>/dev/null
	done
	popd
}

main $@
