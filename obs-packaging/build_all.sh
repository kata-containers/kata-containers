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


script_dir=$(dirname "$0")
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

cd "$script_dir"

OBS_API="https://api.opensuse.org"

if [ -n "${OBS_USER:-}" ] && [ -n "${OBS_PASS:-}" ] && [ ! -e "${OSCRC:-}" ]; then
	echo "Creating  ${OSCRC} with user $OBS_USER"
	cat << eom > "${OSCRC}"
[general]
apiurl = ${OBS_API}
[${OBS_API}]
user = ${OBS_USER}
pass = ${OBS_PASS}
eom
fi

if [ -n "${PUSH}" ]; then
	# push to obs
	PUSH_TO_OBS="-p"
elif [ -n "${LOCAL}" ]; then
	# local build
	PUSH_TO_OBS="-l"
fi

for p in "${projects[@]}"; do
	pushd "$p" >> /dev/null
	echo "update ${p}"
	bash ./update.sh "${PUSH_TO_OBS}" -v
	popd >> /dev/null
done
