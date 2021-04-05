#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"
export branch="${branch:-main}"

# Clones the tests repository and checkout to the branch pointed out by
# the global $branch variable.
# If the clone exists and `CI` is exported then it does nothing. Otherwise
# it will clone the repository or `git pull` the latest code.
#
clone_tests_repo()
{
	if [ -d "$tests_repo_dir" ]; then
		[ -n "$CI" ] && return
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
	bash "$tests_repo_dir/.ci/static-checks.sh" "github.com/kata-containers/kata-containers"
}

run_go_test()
{
	clone_tests_repo
	bash "$tests_repo_dir/.ci/go-test.sh"
}
