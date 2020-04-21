#!/usr/bin/env bash
#
# Copyright (c) 2019 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

function install_yq() {
	local cidir=$(dirname "${BASH_SOURCE[0]}")
	# source lib.sh to make sure GOPATH is set
	source "${cidir}/lib.sh"
	[ -x  "${GOPATH}/bin/yq" ] && return

	local yq_path="${GOPATH}/bin/yq"
	local yq_pkg="github.com/mikefarah/yq"
	local goos="$(uname -s)"
	local goarch="$(${cidir}/kata-arch.sh --golang)"

	mkdir -p "${GOPATH}/bin"

	# Stick to a specific version. Same used in
	# runtime and osbuilder repos.
	local yq_version=3.1.0

	## NOTE: ${var,,} => gives lowercase value of var
	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_${goos,,}_${goarch}"
	curl -o "${yq_path}" -LSsf "${yq_url}"
	chmod +x "${yq_path}"
	echo "Installed $(${yq_path} --version)"

	if ! command -v "${yq_path}" >/dev/null; then
		die "Cannot not get ${yq_path} executable"
	fi
}

install_yq
