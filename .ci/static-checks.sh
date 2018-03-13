#!/bin/bash

# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Central script to run all static checks.
#   This script should be called by all other repositories to ensure
#   there is only a single source of all static checks.

set -e

check_commits()
{
	# Since this script is called from another repositories directory,
	# ensure the utility is built before running it.
	local self="$GOPATH/src/github.com/kata-containers/tests"
	(cd "$self" && make checkcommits)

	# Check the commits in the branch
	{
		checkcommits \
			--need-fixes \
			--need-sign-offs \
			--ignore-fixes-for-subsystem "release" \
			--verbose; \
			rc="$?";
	} || true

	if [ "$rc" -ne 0 ]
	then
		cat >&2 <<-EOT
	ERROR: checkcommits failed. See the document below for help on formatting
	commits for the project.

		https://github.com/kata-containers/community/blob/master/CONTRIBUTING.md#patch-format

EOT
		exit 1
	fi
}

check_go()
{
	local go_packages

	# Note: the vendor filtering is required for versions of go older than 1.9
	go_packages=$(go list ./... 2>/dev/null | grep -v "/vendor/" || true)

	# Ignore the runtime repo which uses submodules. The runtimes it
	# imports are assumed to be tested independently so do not (and should
	# not) need to be re-tested here.
	local runtime_repo="github.com/kata-containers/runtime"

	[ -e ".gitmodules" ] && go_packages=$(echo "$go_packages" |\
		grep -v "$runtime_repo" || true)

	[ -z "$go_packages" ] && return

	# Run golang checks
	if [ ! "$(command -v gometalinter)" ]
	then
		go get github.com/alecthomas/gometalinter
		gometalinter --install --vendor
	fi

	# Ignore vendor directories
	# Note: There is also a "--vendor" flag which claims to do what we want, but
	# it doesn't work :(
	local linter_args="--exclude=\"\\bvendor/.*\""

	# Check test code too
	linter_args+=" --tests"

	# Ignore auto-generated protobuf code.
	#
	# Note that "--exclude=" patterns are *not* anchored meaning this will apply
	# anywhere in the tree.
	linter_args+=" --exclude=\"protocols/grpc/.*\.pb\.go\""

	# When running the linters in a CI environment we need to disable them all
	# by default and then explicitly enable the ones we are care about. This is
	# necessary since *if* gometalinter adds a new linter, that linter may cause
	# the CI build to fail when it really shouldn't. However, when this script is
	# run locally, all linters should be run to allow the developer to review any
	# failures (and potentially decide whether we need to explicitly enable a new
	# linter in the CI).
	#
	# Developers may set KATA_DEV_MODE to any value for the same behaviour.
	[ "$CI" = true ] || [ -n "$KATA_DEV_MODE" ] && linter_args+=" --disable-all"

	[ "$TRAVIS_GO_VERSION" != "tip" ] && linter_args+=" --enable=gofmt"

	linter_args+=" --enable=misspell"
	linter_args+=" --enable=vet"
	linter_args+=" --enable=ineffassign"
	linter_args+=" --enable=gocyclo"
	linter_args+=" --cyclo-over=15"
	linter_args+=" --enable=golint"
	linter_args+=" --deadline=600s"
	linter_args+=" --enable=structcheck"
	linter_args+=" --enable=unused"
	linter_args+=" --enable=staticcheck"
	linter_args+=" --enable=maligned"

	eval gometalinter "${linter_args}" ./...
}

check_commits
check_go
