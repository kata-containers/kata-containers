#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

readonly script_dir="$( cd "$( dirname "${BASH_SOURCE[0]}" )" && pwd )"
readonly script_name="$(basename "${BASH_SOURCE[0]}")"

readonly tmp_dir=$(mktemp -t -d pr-bump.XXXX)
readonly hub_bin="${tmp_dir}/hub-bin"
readonly organization="kata-containers"
PUSH="false"
GOPATH=${GOPATH:-${HOME}/go}

cleanup (){
	[ -d "${tmp_dir}" ] && rm -rf "${tmp_dir}"
}

trap cleanup EXIT

die()
{
	msg="$*"
	echo "ERROR: ${msg}" >&2
	exit 1
}

info()
{
	msg="$*"
	echo "INFO: ${msg}" >&2
}

build_hub() {
	info "Get hub"
	local hub_repo="github.com/github/hub"
	local hub_repo_dir="${GOPATH}/src/${hub_repo}"
	[ -d "${hub_repo_dir}" ]||  git clone --quiet --depth 1 "https://${hub_repo}.git" "${hub_repo_dir}"
	pushd "${hub_repo_dir}" >> /dev/null
	git checkout master
	git pull
	./script/build -o "${hub_bin}"
	popd >> /dev/null
}

get_changes() {
	local current_version=$1
	[ -n "${current_version}" ] || die "current version not provided"

	changes=$(git log --oneline "${current_version}..HEAD") || die "failed to get logs"
	if [ "${changes}" == "" ]; then
		echo "Version bump no changes"
		return
	fi

	# list all PRs merged from $current_version to HEAD
	git log --merges "${current_version}..HEAD" | awk '/Merge pull/{getline; getline;print }' | while read pr
	do
		echo "- ${pr}"
	done

	echo ""

	# list all commits added in this new version.
	git log --oneline "${current_version}..HEAD" --no-merges
}

generate_commit() {
	local new_version=$1
	local current_version=$2

	[ -n "$new_version" ] || die "no new version"
	[ -n "$current_version" ] || die "no current version"

	printf "release: Kata Containers %s\n\n" "${new_version}"

	get_changes "$current_version"
}

bump_repo() {
	repo=$1
	new_version=$2
	[ -n "${repo}" ] || die "repository not provided"
	[ -n "$new_version" ] || die "no new version"
	remote_github="https://github.com/${organization}/${repo}.git"
	info "Update $repo to version $new_version"

	info "remote: ${remote_github}"

	git clone --quiet "${remote_github}"

	pushd "${repo}" >> /dev/null

	# All repos we build should have a VERSION file
	[ -f "VERSION" ] || die "VERSION file not found "
	current_version="$(cat ./VERSION | grep -v '#')"

	info "Creating PR message"
	notes_file=notes.md
	cat << EOT > "${notes_file}"
# Kata Containers ${new_version}

$(get_changes "$current_version")

EOT

	info "Updating VERSION file"
	echo "${new_version}" > VERSION
	branch="${new_version}-branch-bump"
	git checkout -b "${branch}" master
	git add -u
	info "Creating commit with new changes"
	commit_msg="$(generate_commit $new_version $current_version)"
	git  commit -s -m "${commit_msg}"

	if [[ "${PUSH}" == "true" ]]; then
		build_hub
		info "Forking remote"
		${hub_bin} fork --remote-name=fork
		info "Push to fork"
		${hub_bin} push fork -f "${branch}"
		info "Create PR"
		out=""
		out=$("${hub_bin}" pull-request -F "${notes_file}" 2>&1) || echo "$out" |  grep "A pull request already exists"
	fi
	popd >> /dev/null
}

usage(){
	exit_code="$1"
	cat <<EOT
Usage:
	${script_name} [options] <args>
Args:
	<repository-name> : Name of repository to fork and send PR from github.com/${organization}
	<new-version>    : New version to bump the repository
Example:
	${script_name} 1.10
Options
	-h        : Show this help
	-p        : create a PR
EOT
	exit "$exit_code"
}

while getopts "hp" opt
do
	case $opt in
		h)	usage 0 ;;
		p)	PUSH="true" ;;
	esac
done

shift $(( $OPTIND - 1 ))

repo=${1:-}
new_version=${2:-}
[ -n "${repo}" ] || (echo "ERROR: repository not provided" && usage 1)
[ -n "$new_version" ] || (echo "ERROR: no new version" && usage 1 )

pushd "$tmp_dir" >> /dev/null
bump_repo "${repo}" "${new_version}"
popd >> /dev/null
