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

run_ci_fast_return()
{
	clone_tests_repo
	# Make sure we have the targeting branch
	git remote set-branches --add origin "${branch}"
	git fetch -a

	# Check if we can fastpath return/skip the CI
	# Work around the 'set -e' dying if the check fails by using a bash
	# '{ group command }' to encapsulate.
	{
		if [ "${PR_NUMBER:-}"  != "" ]; then
			echo "Testing a PR ${PR_NUMBER} check if can fastpath return/skip"
			"${tests_repo_dir}/.ci/ci-fast-return.sh"
			ret=$?
		else
			echo "not a PR will run all the CI"
			ret=1
		fi
	} || true

	if [ "$ret" -eq 0 ]; then
		echo "Short circuit fast path skipping the rest of the CI."
		exit 0
	else
		exit 1
	fi
}
