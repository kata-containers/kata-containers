#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

readonly script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
readonly script_name="$(basename "${BASH_SOURCE[0]}")"

readonly tmp_dir=$(mktemp -t -d pr-bump.XXXX)
readonly organization="kata-containers"
PUSH="false"
GOPATH=${GOPATH:-${HOME}/go}

source "${script_dir}/../scripts/lib.sh"

cleanup() {
	[ -d "${tmp_dir}" ] && rm -rf "${tmp_dir}"
}

trap cleanup EXIT

handle_error() {
	local exit_code="${?}"
	local line_number="${1:-}"
	echo "Failed at $line_number: ${BASH_COMMAND}"
	exit "${exit_code}"
}
trap 'handle_error $LINENO' ERR

get_changes() {
	local current_version=$1
	[ -n "${current_version}" ] || die "current version not provided"
	if [ "${current_version}" == "new" ];then
		echo "Starting to version this repository"
		return
	fi

	# If for some reason there is not a tag this could fail
	# better fail and write the error in the PR
	if ! changes=$(git log --oneline "${current_version}..HEAD"); then
		echo "failed to get logs"
	fi
	if [ "${changes}" == "" ]; then
		echo "Version bump no changes"
		return
	fi

	# list all PRs merged from $current_version to HEAD
	git log --merges "${current_version}..HEAD" | awk '/Merge pull/{getline; getline;print }' | while read pr; do
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
	local repo="${1:-}"
	local new_version="${2:-}"
	local target_branch="${3:-}"
	[ -n "${repo}" ] || die "repository not provided"
	[ -n "${new_version}" ] || die "no new version"
	[ -n "${target_branch}" ] || die "no target branch"
	local remote_github="https://github.com/${organization}/${repo}.git"
	info "Update $repo to version $new_version"

	info "remote: ${remote_github}"

	git clone --quiet "${remote_github}"

	pushd "${repo}" >>/dev/null

	branch="${new_version}-branch-bump"
	git fetch origin "${target_branch}"
	git checkout "origin/${target_branch}" -b "${branch}"

	# All repos we build should have a VERSION file
	if [ ! -f "VERSION" ]; then
		current_version="new"
		echo "${new_version}" >VERSION
	else
		current_version="$(grep -v '#' ./VERSION)"

		info "Updating VERSION file"
		echo "${new_version}" >VERSION
		if git diff --exit-code; then
			info "${repo} already in version ${new_version}"
			cat VERSION
			return 0
		fi
	fi

	if [ "${repo}" == "kata-containers" ]; then
		info "Updating kata-deploy / kata-cleanup image tags"
		sed -i "s#katadocker/kata-deploy:${current_version}#katadocker/kata-deploy:${new_version}#g" tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml
		sed -i "s#katadocker/kata-deploy:${current_version}#katadocker/kata-deploy:${new_version}#g" tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml
		git diff

		git add tools/packaging/kata-deploy/kata-deploy/base/kata-deploy.yaml
		git add tools/packaging/kata-deploy/kata-cleanup/base/kata-cleanup.yaml
	fi

	info "Creating PR message"
	notes_file=notes.md
	cat <<EOT >"${notes_file}"
# Kata Containers ${new_version}

$(get_changes "$current_version")

EOT
	cat "${notes_file}"

	if (echo "${current_version}" | grep "alpha") && (echo "${new_version}" | grep -v "alpha");then
		info "update move from alpha, check if new version is rc0"
		if echo "$new_version" | grep -v "rc0"; then
			die "bump should be from alpha to rc0"
		fi
		info "OK"
	fi

	git add VERSION
	info "Creating commit with new changes"
	commit_msg="$(generate_commit $new_version $current_version)"
	git commit -s -m "${commit_msg}"

	if [[ ${PUSH} == "true" ]]; then
		build_hub
		info "Forking remote"
		${hub_bin} fork --remote-name=fork
		info "Push to fork"
		${hub_bin} push fork -f "${branch}"
		info "Create PR"
		out=""
		out=$("${hub_bin}" pull-request -b "${target_branch}" -F "${notes_file}" 2>&1) || echo "$out" | grep "A pull request already exists"
	fi
	popd >>/dev/null
}

usage() {
	exit_code="$1"
	cat <<EOT
Usage:
	${script_name} [options] <args>
Args:
	<new-version>     : New version to bump the repository
	<target-branch>   : The base branch to create to PR
Example:
	${script_name} 1.10
Options
	-h        : Show this help
	-p        : create a PR
EOT
	exit "$exit_code"
}

repos=(
	"kata-containers"
	"tests"
)

main(){
	while getopts "hp" opt; do
		case $opt in
			h) usage 0 ;;
			p) PUSH="true" ;;
		esac
	done

	shift $((OPTIND - 1))


	new_version=${1:-}
	target_branch=${2:-}
	[ -n "${new_version}" ] || { echo "ERROR: no new version" && usage 1; }
	[ -n "${target_branch}" ] || die "no target branch"
	for repo in "${repos[@]}"
	do
		pushd "$tmp_dir" >>/dev/null
		bump_repo "${repo}" "${new_version}" "${target_branch}"
		popd >>/dev/null
	done

}
main $@
