#!/usr/bin/env bash
#
# Copyright (c) 2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

function add_kata_bot_info() {
	echo "Adding user name and email to the local git repo"

	git config user.email "katacontainersbot@gmail.com"
	git config user.name "Kata Containers Bot"
}

function rebase_atop_of_the_latest_target_branch() {
	if [[ -n "${TARGET_BRANCH}" ]]; then
		echo "Rebasing atop of the latest ${TARGET_BRANCH}"
		if ! git rebase "origin/${TARGET_BRANCH}"; then
			if [[ -n "${GITHUB_WORKSPACE}" ]] ; then
				echo "Rebase failed, cleaning up the local repository and exiting"
				cd "${GITHUB_WORKSPACE}"/..
				sudo rm -rf "${GITHUB_WORKSPACE}"
			else
				echo "Rebase failed, exiting"
			fi
			exit 1
		fi
	fi
}

function main() {
    action="${1:-}"

    add_kata_bot_info

    case "${action}" in
	rebase-atop-of-the-latest-target-branch) rebase_atop_of_the_latest_target_branch;;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
