#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# If we fail for any reason a message will be displayed
die() {
	msg="$*"
	echo "ERROR: $msg" >&2
	exit 1
}

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"

clone_tests_repo() {
	# KATA_CI_NO_NETWORK is (has to be) ignored if there is
	# no existing clone.
	if [ -d "${tests_repo_dir}" ] && [ -n "${KATA_CI_NO_NETWORK:-}" ]; then
		return
	fi

	go get -d -u "$tests_repo" || true
}

install_yq() {
	path=$1
	local yq_path=${path}/yq
	local yq_pkg="github.com/mikefarah/yq"
	[ -x "${yq_path}" ] && return

	case "$(arch)" in
	"aarch64")
		goarch=arm64
		;;

	"x86_64")
		goarch=amd64
		;;

	"ppc64le")
		goarch=ppc64le
		;;

	"s390x")
		goarch=s390x
		;;

	"*")
		echo "Arch $(arch) not supported"
		exit
		;;
	esac

	mkdir -p "${path}"

	# Workaround to get latest release from github (to not use github token).
	# Get the redirection to latest release on github.
	yq_latest_url=$(curl -Ls -o /dev/null -w %{url_effective} "https://${yq_pkg}/releases/latest")
	# The redirected url should include the latest release version
	# https://github.com/mikefarah/yq/releases/tag/<VERSION-HERE>
	yq_version=$(basename "${yq_latest_url}")

	local yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_linux_${goarch}"
	curl -o "${yq_path}" -L ${yq_url}
	chmod +x ${yq_path}
}
