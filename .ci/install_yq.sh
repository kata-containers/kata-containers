#!/bin/bash
#
# Copyright (c) 2019 SUSE LLC
#
# SPDX-License-Identifier: Apache-2.0
#

set -eu

function install_yq() {
	local cidir=$(dirname "${BASH_SOURCE[0]}")
	# source lib.sh to make sure GOPATH is set
	source ${cidir}/lib.sh
	[ -x  "${GOPATH}/bin/yq" ] && return

	local yq_path="${GOPATH}/bin/yq"
	local yq_pkg="github.com/mikefarah/yq"
	local goos="$(uname -s)"
	local goarch="$(${cidir}/kata-arch.sh --golang)"

	mkdir -p "${GOPATH}/bin"

	# Workaround to get latest release from github (to not use github token).
	# Get the redirection to latest release on github.
	yq_latest_url=$(curl -Ls -o /dev/null -w %{url_effective} "https://${yq_pkg}/releases/latest")
	# The redirected url should include the latest release version
	# https://github.com/mikefarah/yq/releases/tag/<VERSION-HERE>
	yq_version=$(basename "${yq_latest_url}")

	## NOTE: ${var,,} => gives lowercase value of var
	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_${goos,,}_${goarch}"
	curl -o "${yq_path}" -LSsf ${yq_url}
	chmod +x ${yq_path}

	if ! command -v "${yq_path}" >/dev/null; then
		die "Cannot not get ${yq_path} executable"
	fi
}

install_yq
