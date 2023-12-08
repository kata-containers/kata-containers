#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o nounset

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"
export branch="${target_branch:-main}"

# Clones the tests repository and checkout to the branch pointed out by
# the global $branch variable.
# If the clone exists and `CI` is exported then it does nothing. Otherwise
# it will clone the repository or `git pull` the latest code.
#
clone_tests_repo()
{
	if [ -d "$tests_repo_dir" ]; then
		[ -n "${CI:-}" ] && return
		# git config --global --add safe.directory will always append
		# the target to .gitconfig without checking the existence of
		# the target, so it's better to check it before adding the target repo.
		local sd="$(git config --global --get safe.directory ${tests_repo_dir} || true)"
		if [ -z "${sd}" ]; then
			git config --global --add safe.directory ${tests_repo_dir}
		fi
		pushd "${tests_repo_dir}"
		git checkout "${branch}"
		git pull
		popd
	else
		git clone -q "https://${tests_repo}" "$tests_repo_dir"
		pushd "${tests_repo_dir}"
		git checkout "${branch}"
		popd
	fi
}

run_static_checks()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "$tests_repo_dir/.ci/static-checks.sh" "$@"
}

run_docs_url_alive_check()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	bash "$tests_repo_dir/.ci/static-checks.sh" --docs --all "github.com/kata-containers/kata-containers"
}

run_get_pr_changed_file_details()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a
	source "$tests_repo_dir/.ci/lib.sh"
	get_pr_changed_file_details
}

# Check if the 1st argument version is greater than and equal to 2nd one
# Version format: [0-9]+ separated by period (e.g. 2.4.6, 1.11.3 and etc.)
#
# Parameters:
#	$1	- a version to be tested
#	$2	- a target version
#
# Return:
# 	0 if $1 is greater than and equal to $2
#	1 otherwise
version_greater_than_equal() {
	local current_version=$1
	local target_version=$2
	smaller_version=$(echo -e "$current_version\n$target_version" | sort -V | head -1)
	if [ "${smaller_version}" = "${target_version}" ]; then
		return 0
	else
		return 1
	fi
}
