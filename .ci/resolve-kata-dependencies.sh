#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

# Repositories needed for building the kata containers project.
agent_repo="${agent_repo:-github.com/kata-containers/agent}"
proxy_repo="${proxy_repo:-github.com/kata-containers/proxy}"
runtime_repo="${runtime_repo:-github.com/kata-containers/runtime}"
shim_repo="${shim_repo:-github.com/kata-containers/shim}"
tests_repo="${tests_repo:-github.com/kata-containers/tests}"

apply_depends_on() {
	pushd "${GOPATH}/src/${kata_repo}"
	label_lines=$(git log --format=%s%b master.. | grep "Depends-on:" || true)
	if [ "${label_lines}" == "" ]; then
		popd
		return 0
	fi

	nb_lines=$(echo "${label_lines}" | wc -l)

	repos_found=()
	for i in $(seq 1 "${nb_lines}")
	do
		label_line=$(echo "${label_lines}" | sed "${i}q;d")
		label_str=$(echo "${label_line}" | cut -d ':' -f2)
		repo=$(echo "${label_str}" | tr -d '[:space:]' | cut -d'#' -f1)
		if [[ "${repos_found[@]}" =~ "${repo}" ]]; then
			echo "Repository $repo was already defined in a 'Depends-on:' tag."
			echo "Only one repository per tag is allowed."
			return 1
		fi
		repos_found+=("$repo")
		pr_id=$(echo "${label_str}" | cut -d'#' -f2)

		echo "This PR depends on repository: ${repo} and pull request: ${pr_id}"
		if [ ! -d "${GOPATH}/src/${repo}" ]; then
			go get -d "$repo" || true
		fi

		pushd "${GOPATH}/src/${repo}"
		echo "Fetching pull request: ${pr_id} for repository: ${repo}"
		dependency_branch="p${pr_id}"
		git fetch origin "pull/${pr_id}/head:${dependency_branch}" && \
			git checkout "${dependency_branch}" && \
			git rebase "origin/${branch}"
		popd
	done

	popd
}

clone_repos() {
	local kata_repos=( "${agent_repo}" "${proxy_repo}" "${runtime_repo}" "${shim_repo}" "${tests_repo}" )
	for repo in "${kata_repos[@]}"
	do
		echo "Cloning ${repo}"
		go get -d "${repo}" || true
		repo_dir="${GOPATH}/src/${repo}"

		# The tests repository is cloned and checkout to the PR branch directly in
		# the CI configuration (e.g. jenkins file or zuul config), because we want
		# to have latest changes of this repository, since the job starts. So we
		# need to verify if we are already in the PR branch, before trying to
		# fetch the same branch.
		if [ ${repo} == ${tests_repo} ]
		then
		current_branch=$(git rev-parse --abbrev-ref HEAD)
			if [ "${current_branch}" == "${pr_branch}" ]
			then
				echo "Already on branch ${current_branch}"
				return
			fi
		fi

		pushd "${repo_dir}"
		if [ "${repo}" == "${kata_repo}" ]
		then
			git fetch origin "pull/${pr_number}/head:${pr_branch}"
			echo "Checking out to ${pr_branch} branch"
			git checkout "${pr_branch}"
			echo "... and rebasing with origin/${branch}"
			git rebase "origin/${branch}"
		else
			echo "Checking out to ${branch}"
			git fetch origin && git checkout "$branch"
		fi
		popd
	done
}

main() {
	clone_repos
	apply_depends_on
}

main
