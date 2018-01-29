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
	checkcommits \
		--need-fixes \
		--need-sign-offs \
		--ignore-fixes-for-subsystem "release" \
		--verbose
}

check_go()
{
	local go_packages

	go_packages=$(go list ./... 2>/dev/null || true)

	[ -z "$go_packages" ] && exit 0

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

	linter_args+=" --enable=misspell"
	linter_args+=" --enable=vet"
	linter_args+=" --enable=ineffassign"
	linter_args+=" --enable=gofmt"
	linter_args+=" --enable=gocyclo"
	linter_args+=" --cyclo-over=15"
	linter_args+=" --enable=golint"
	linter_args+=" --deadline=600s"

	eval gometalinter "${linter_args}" ./...
}

check_commits
check_go
