#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

tmp_dir=$(mktemp -d -t tag-repos-tmp.XXXXXXXXXX)
script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
script_name="$(basename "${BASH_SOURCE[0]}")"
OWNER=${OWNER:-"kata-containers"}
PROJECT="Kata Containers"
PUSH="${PUSH:-"false"}"
commit="master"
readonly URL_RAW_FILE="https://raw.githubusercontent.com/${OWNER}"
#The runtime version is used as reference of latest release
readonly kata_version=$(curl -Ls "${URL_RAW_FILE}/runtime/${commit}/VERSION" | grep -v -P "^#")

function usage() {

	cat <<EOT
Usage: ${script_name} [options] <args>
This script creates a new release for ${PROJECT}.
It tags and create release for:
EOT
	for r in "${repos[@]}"; do
		echo "  - ${r}"
	done

	cat <<EOT

Args:
status : Get Current ${PROJECT} tags status
tag    : Create tags for ${PROJECT}

Options:
-h : Show this help
-p : push tags

EOT

}

finish() {
	rm -rf "$tmp_dir"
}

trap finish EXIT

die() {
	echo >&2 "ERROR: $*"
	exit 1
}

info() {
	echo "INFO: $*"
}

repos=(
	"agent"
	"ksm-throttler"
	"osbuilder"
	"proxy"
	"runtime"
	"shim"
)

check_versions() {

	info "Tagging ${PROJECT} with version ${kata_version}"
	info "Check all repos has version ${kata_version} in VERSION file"

	for repo in "${repos[@]}"; do
		repo_version=$(curl -Ls "${URL_RAW_FILE}/${repo}/${commit}/VERSION" | grep -v -P "^#")
		info "${repo} is in $repo_version"
		[ "${repo_version}" == "${kata_version}" ] || die "${repo} is not in version ${kata_version}"
	done
}

tag_repos() {

	info "Creating tag ${kata_version} in all repos"
	for repo in "${repos[@]}"; do
		git clone --quiet "https://github.com/${OWNER}/${repo}.git"
		pushd "${repo}"
		git remote set-url --push origin "git@github.com:${OWNER}/${repo}.git"
		git fetch origin --tags
		if git rev-parse -q --verify "refs/tags/${kata_version}"; then
			info "$repo already has tag "
		else
			info "Creating tag ${kata_version} for ${repo}"
			git tag -a "${kata_version}" -s -m "${PROJECT} release ${kata_version}"
		fi
		popd
	done
}

push_tags() {
	info "Pushing tags to repos"
	for repo in "${repos[@]}"; do
		pushd "${repo}"
		git push origin "${kata_version}"
		popd
	done
}

while getopts "hp" opt; do
	case $opt in
	h) usage && exit 0 ;;
	p) PUSH="true" ;;
	esac
done
shift $(($OPTIND - 1))

subcmd=${1:-""}

[ -z "${subcmd}" ] && usage && exit 0

pushd "${tmp_dir}"

case "${subcmd}" in
status)
	check_versions
	;;
tag)
	check_versions
	# Tag versions that does not have VERSIONS file
	# But we want to know the version compatible with a kata release.
	repos+=("tests")
	tag_repos
	if [ "${PUSH}" == "true" ]; then
		push_tags
	else
		info "tags not pushed, use -p option to push the tags"
	fi
	;;
*)
	usage && die "Invalid argument ${subcmd}"
	;;

esac

popd
