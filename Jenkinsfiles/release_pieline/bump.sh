#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

export CI="true"

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

readonly script_name="$(basename "${BASH_SOURCE[0]}")"

function handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

install_go() {
	echo "Installing go"
	export GOROOT="/usr/local/go"
	# shellcheck disable=SC2016
	echo 'export PATH=$PATH:'"${GOROOT}/bin" | sudo tee -a /etc/profile
	export PATH="$PATH:${GOROOT}/bin"

	export GOPATH="${WORKSPACE}/go"
	mkdir -p "${GOPATH}"

	tests_repo="github.com/kata-containers/tests"
	tests_repo_dir="${GOPATH}/src/${tests_repo}"
	# shellcheck disable=SC2046
	mkdir -p $(dirname "${tests_repo_dir}")
	[ -d "${tests_repo_dir}" ] || git clone "https://${tests_repo}.git" "${tests_repo_dir}"
	"${GOPATH}/src/${tests_repo}/.ci/install_go.sh" -p -f
	go version
}

install_docker() {
	echo "Installing docker"
	sudo -E apt-get --no-install-recommends install -y apt-transport-https apt-utils ca-certificates software-properties-common
	curl -sL https://download.docker.com/linux/ubuntu/gpg | sudo apt-key add -
	arch=$(dpkg --print-architecture)
	sudo -E add-apt-repository "deb [arch=${arch}] https://download.docker.com/linux/ubuntu $(lsb_release -cs) stable"
	sudo -E apt-get update
	sudo -E apt-get --no-install-recommends install -y docker-ce
}

setup_git() {
	echo "configuring git"
	git config --global user.email "katabuilder@katacontainers.io"
	git config --global user.name "katabuilder"
	export HUB_PROTOCOL=https
}

bump_kata() {
	new_version=${1:-}
	branch=${2:-}
	[ -n "${new_version}" ]
	[ -n "${branch}" ]
	readonly packaging_repo="github.com/kata-containers/packaging"
	readonly packaging_repo_dir="${GOPATH}/src/${packaging_repo}"
	[ -d "${packaging_repo_dir}" ] || git clone "https://${packaging_repo}.git" "${packaging_repo_dir}"

	cd "${packaging_repo_dir}/release"
	./update-repository-version.sh -p "$new_version" "$branch"
}

setup() {
	setup_git
	install_go
	install_docker
}

usage() {
	exit_code="$1"
	cat <<EOT
Usage:
${script_name}  <args>
Args:
	<new-version> : new version to bump kata
	<branch> : branch target
Example:
	${script_name} 1.10
EOT

	exit "$exit_code"
}

main() {
	new_version=${1:-}
	branch=${2:-}
	[ -n "${new_version}" ] || usage 1
	[ -n "${branch}" ] || usage 1
	echo "Start Release ${new_version} for branch ${branch}"
	setup
	bump_kata "${new_version}" "${branch}"
}

main $@
