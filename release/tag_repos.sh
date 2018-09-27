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
branch="master"
readonly URL_RAW_FILE="https://raw.githubusercontent.com/${OWNER}"
#The runtime version is used as reference of latest release
# This is set to the right value later.
kata_version=""

source "${script_dir}/../scripts/lib.sh"

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
-b <branch>: branch were will check the version.
-h         : Show this help
-p         : push tags

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
	"proxy"
	"runtime"
	"shim"
)
# Tag versions that do not have a VERSIONS file
# But we want to know the version compatible with a kata release.
repos_not_versions=(
	"tests"
	"packaging"
	"osbuilder"
	"ksm-throttler"
)

# List of repositories that does not have stable branch.
# We want to do is just push the tags to master branch
# since we don't maintain a seperate branch for following repos.
not_stable_branch=(
	"packaging"
	"osbuilder"
	"ksm-throttler"
)

check_versions() {

	info "Tagging ${PROJECT} with version ${kata_version}"
	info "Check all repos has version ${kata_version} in VERSION file"

	for repo in "${repos[@]}"; do
		repo_version=$(curl -Ls "${URL_RAW_FILE}/${repo}/${branch}/VERSION" | grep -v -P "^#")
		info "${repo} is in $repo_version"
		[ "${repo_version}" == "${kata_version}" ] || die "${repo} is not in version ${kata_version}"
	done
}

tag_repos() {

	info "Creating tag ${kata_version} in all repos"
	for repo in "${repos[@]}"; do
		git clone --quiet "https://github.com/${OWNER}/${repo}.git"
		pushd "${repo}" >>/dev/null
		git remote set-url --push origin "git@github.com:${OWNER}/${repo}.git"
		git fetch origin --tags
		tag="$kata_version"
		if [[ ! ${not_stable_branch[*]} =~ ${repo} ]]; then
			info "Checkout to ${branch} in ${repo}"
			git checkout "${branch}"
		else
			info "Checkout(${branch}) not need for ${repo}"
		fi
		[[ "packaging" == "${repo}" ]] && tag="${tag}-kernel-config"
		if git rev-parse -q --verify "refs/tags/${tag}"; then
			info "$repo already has tag "
		else
			info "Creating tag ${tag} for ${repo}"
			git tag -a "${tag}" -s -m "${PROJECT} release ${tag}"
		fi
		popd >>/dev/null
	done
}

push_tags() {
	info "Pushing tags to repos"
	build_hub
	for repo in "${repos[@]}"; do
		pushd "${repo}" >>/dev/null
		tag="$kata_version"
		[[ "packaging" == "${repo}" ]] && tag="${tag}-kernel-config"
		info "Push tag ${tag} for ${repo}"
		git push origin "${tag}"
		create_github_release "${PWD}" "${tag}"
		popd >>/dev/null
	done
}

create_github_release() {
	repo_dir=${1:-}
	tag=${2:-}
	[ -d "${repo_dir}" ] || die "No repository directory"
	[ -n "${tag}" ] || die "No tag specified"
	if ! "${hub_bin}" release | grep -q "^${tag}$"; then
		info "Creating Github release"
		"${hub_bin}" -C "${repo_dir}" release create -m "${PROJECT} ${tag}" "${tag}"
	else
		info "Github release already created"
	fi
}

while getopts "b:hp" opt; do
	case $opt in
	b) branch="${OPTARG}" ;;
	h) usage && exit 0 ;;
	p) PUSH="true" ;;
	esac
done
shift $((OPTIND - 1))

subcmd=${1:-""}
kata_version=$(curl -Ls "${URL_RAW_FILE}/runtime/${branch}/VERSION" | grep -v -P "^#")

[ -z "${subcmd}" ] && usage && exit 0

pushd "${tmp_dir}" >>/dev/null

case "${subcmd}" in
status)
	check_versions
	;;
tag)
	check_versions
	repos+=("${repos_not_versions[@]}")
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

popd >>/dev/null
