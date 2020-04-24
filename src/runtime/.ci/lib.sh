#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="$GOPATH/src/$tests_repo"

clone_tests_repo()
{
	# KATA_CI_NO_NETWORK is (has to be) ignored if there is
	# no existing clone.
	if [ -d "$tests_repo_dir" -a -n "$KATA_CI_NO_NETWORK" ]
	then
		return
	fi

	go get -d -u "$tests_repo" || true
	if [ -n "${TRAVIS_BRANCH:-}" ]; then
		( cd "${tests_repo_dir}" && git checkout "${TRAVIS_BRANCH}" )
	fi
}

run_static_checks()
{
	clone_tests_repo
	bash "$tests_repo_dir/.ci/static-checks.sh"  "github.com/kata-containers/runtime"
}

run_go_test()
{
	clone_tests_repo
	bash "$tests_repo_dir/.ci/go-test.sh"
}
