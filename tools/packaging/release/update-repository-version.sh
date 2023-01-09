#!/usr/bin/env bash
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
	local current_version="$1"
	[ -n "${current_version}" ] || die "current version not provided"

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

generate_kata_deploy_commit() {
       local new_version="$1"
       [ -n "$new_version" ] || die "no new version"

       printf "release: Adapt kata-deploy for %s" "${new_version}"

       printf "\n
kata-deploy files must be adapted to a new release.  The cases where it
happens are when the release goes from -> to:
* main -> stable:
  * kata-deploy-stable / kata-cleanup-stable: are removed

* stable -> stable:
  * kata-deploy / kata-cleanup: bump the release to the new one.

There are no changes when doing an alpha release, as the files on the
\"main\" branch always point to the \"latest\" and \"stable\" tags."
}

generate_revert_kata_deploy_commit() {
       local new_version="$1"
       [ -n "$new_version" ] || die "no new version"

       printf "release: Revert kata-deploy changes after %s release" "${new_version}"

       printf "\n
As %s has been released, let's switch the kata-deploy / kata-cleanup
tags back to \"latest\", and re-add the kata-deploy-stable and the
kata-cleanup-stable files." "${new_version}"
}

generate_commit() {
	local new_version="$1"
	local current_version="$2"

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

	local kata_deploy_dir="tools/packaging/kata-deploy"
	local kata_deploy_base="${kata_deploy_dir}/kata-deploy/base"
	local kata_cleanup_base="${kata_deploy_dir}/kata-cleanup/base"
	local kata_deploy_yaml="${kata_deploy_base}/kata-deploy.yaml"
	local kata_cleanup_yaml="${kata_cleanup_base}/kata-cleanup.yaml"
	local kata_deploy_stable_yaml="${kata_deploy_base}/kata-deploy-stable.yaml"
	local kata_cleanup_stable_yaml="${kata_cleanup_base}/kata-cleanup-stable.yaml"

	branch="${new_version}-branch-bump"
	git fetch origin "${target_branch}"
	git checkout "origin/${target_branch}" -b "${branch}"

	local current_version="$(egrep -v '^(#|$)' ./VERSION)"

	info "Updating VERSION file"
	echo "${new_version}" >VERSION
	if git diff --exit-code; then
		info "${repo} already in version ${new_version}"
		return 0
	fi

	if [ "${repo}" == "kata-containers" ]; then
		# Here there are 3 scenarios of what we can do, based on
	        # which branch we're targetting:
		#
		# 1) [main] ------> [main]        NO-OP
		#   "alpha0"       "alpha1"
		#
		#                     +----------------+----------------+
		#                     |      from      |       to       |
		#  -------------------+----------------+----------------+
		#  kata-deploy        | "latest"       | "latest"       |
		#  -------------------+----------------+----------------+
		#  kata-deploy-stable | "stable        | "stable"       |
		#  -------------------+----------------+----------------+
		#
		#
		# 2) [main] ------> [stable]  Update kata-deploy and
		#   "alpha2"         "rc0"    get rid of kata-deploy-stable
		#
		#                     +----------------+----------------+
		#                     |      from      |       to       |
		#  -------------------+----------------+----------------+
		#  kata-deploy        | "latest"       | "latest"       |
		#  -------------------+----------------+----------------+
		#  kata-deploy-stable | "stable"       | REMOVED        |
		#  -------------------+----------------+----------------+
		#
		#
		# 3) [stable] ------> [stable]    Update kata-deploy
		#    "x.y.z"         "x.y.(z+1)"
		#
		#                     +----------------+----------------+
		#                     |      from      |       to       |
		#  -------------------+----------------+----------------+
		#  kata-deploy        | "x.y.z"        | "x.y.(z+1)"    |
		#  -------------------+----------------+----------------+
		#  kata-deploy-stable | NON-EXISTENT   | NON-EXISTENT   |
		#  -------------------+----------------+----------------+

		local registry="quay.io/kata-containers/kata-deploy"

		info "Updating kata-deploy / kata-cleanup image tags"
		local version_to_replace="${current_version}"
		local replacement="${new_version}"
		local need_commit=false
		if [ "${target_branch}" == "main" ];then
			if [[ "${new_version}" =~ "rc" ]]; then
				## We are bumping from alpha to RC, should drop kata-deploy-stable yamls.
				git rm "${kata_deploy_stable_yaml}"
				git rm "${kata_cleanup_stable_yaml}"

				need_commit=true
			fi
		elif [[ ! "${new_version}" =~ "rc" ]]; then
			## We are on a stable branch and creating new stable releases.
			## Need to change kata-deploy / kata-cleanup to use the stable tags.
			if [[ "${version_to_replace}" =~ "rc" ]]; then
				## Coming from "rcX" so from the latest tag.
				version_to_replace="latest"
			fi
			sed -i "s#${registry}:${version_to_replace}#${registry}:${replacement}#g" "${kata_deploy_yaml}"
			sed -i "s#${registry}:${version_to_replace}#${registry}:${replacement}#g" "${kata_cleanup_yaml}"

			git diff

			git add "${kata_deploy_yaml}"
			git add "${kata_cleanup_yaml}"

			need_commit=true
		fi

		if [ "${need_commit}" == "true" ]; then
			info "Creating the commit with the kata-deploy changes"
			local commit_msg="$(generate_kata_deploy_commit $new_version)"
			git commit -s -m "${commit_msg}"
			local kata_deploy_commit="$(git rev-parse HEAD)"
		fi
	fi

	info "Creating PR message"
	notes_file=notes.md
	cat <<EOF >"${notes_file}"
# Kata Containers ${new_version}

$(get_changes "$current_version")

EOF
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
		out=$(LC_ALL=C LANG=C "${hub_bin}" pull-request -b "${target_branch}" -F "${notes_file}" 2>&1) || echo "$out" | grep "A pull request already exists"
	fi

	if [ "${repo}" == "kata-containers" ] && [ "${target_branch}" == "main" ] && [[ "${new_version}" =~ "rc" ]]; then
		reverting_kata_deploy_changes_branch="revert-kata-deploy-changes-after-${new_version}-release"
		git checkout -b "${reverting_kata_deploy_changes_branch}"

		git revert --no-edit ${kata_deploy_commit} >>/dev/null
		commit_msg="$(generate_revert_kata_deploy_commit $new_version)"
		info "Creating the commit message reverting the kata-deploy changes"
		git commit --amend -s -m "${commit_msg}"

		echo "${commit_msg}" >"${notes_file}"
		echo "" >>"${notes_file}"
		echo "Only merge this commit after ${new_version} release is successfully tagged!" >>"${notes_file}"

		if [[ ${PUSH} == "true" ]]; then
			info "Push \"${reverting_kata_deploy_changes_branch}\" to fork"
			${hub_bin} push fork -f "${reverting_kata_deploy_changes_branch}"
			info "Create \"${reverting_kata_deploy_changes_branch}\" PR"
			out=""
			out=$(LC_ALL=C LANG=C "${hub_bin}" pull-request -b "${target_branch}" -F "${notes_file}" 2>&1) || echo "$out" | grep "A pull request already exists"
		fi
	fi

	popd >>/dev/null
}

usage() {
	exit_code="$1"
	cat <<EOF
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
EOF
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


	new_version="${1:-}"
	target_branch="${2:-}"
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
