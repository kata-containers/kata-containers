#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"
export branch="${branch:-2.0-dev}"

clone_tests_repo()
{
	if [ -d "$tests_repo_dir" -a -n "$CI" ]
	then
		return
	fi

	go get -d -u "$tests_repo" || true

	pushd "${tests_repo_dir}" && git checkout "${branch}" && popd
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
