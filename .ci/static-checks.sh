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

# Convert a golang package to a full path
pkg_to_path()
{
	local pkg="$1"

	go list -f '{{.Dir}}' "$pkg"
}

check_go()
{
	local go_packages
	local submodule_packages
	local all_packages

	# List of all golang packages found in all submodules
	#
	# These will be ignored: since they are references to other
	# repositories, we assume they are tested independently in their
	# repository so do not need to be re-tested here.
	submodule_packages=$(mktemp)
	git submodule -q foreach "go list ./..." > "$submodule_packages" || true

	# all packages
	all_packages=$(mktemp)
	go list ./... > "$all_packages" || true

	# List of packages to consider which is defined as:
	#
	#   "all packages" - "submodule packages"
	#
	# Note: the vendor filtering is required for versions of go older than 1.9
	go_packages=$(comm -3 "$all_packages" "$submodule_packages" | grep -v "/vendor/" || true)

	rm -f "$submodule_packages" "$all_packages"

	# No packages to test
	[ -z "$go_packages" ] && return

	local linter="gometalinter"

	# Run golang checks
	if [ ! "$(command -v gometalinter)" ]
	then
		go get github.com/alecthomas/gometalinter
		eval "$linter" --install --vendor
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
	linter_args+=" --enable=varcheck"
	linter_args+=" --enable=unconvert"

	echo -e "INFO: $linter args: '$linter_args'"

	# Non-option arguments other than "./..." are
	# considered to be directories by $linter, not package names.
	# Hence, we need to obtain a list of package directories to check,
	# excluding any that relate to submodules.
	local dirs

	for pkg in $go_packages
	do
		path=$(pkg_to_path "$pkg")

		makefile="${path}/Makefile"

		# perform a basic build since some repos generate code which
		# is required for the package to be buildable (and thus
		# checkable).
		[ -f "$makefile" ] && (cd "$path" && make)

		dirs+=" $path"
	done

	echo -e "INFO: Running $linter checks on the following packages:\n"
	echo "$go_packages"
	echo
	echo "(package paths:$dirs)"

	eval "$linter" "${linter_args}" "$dirs"
}

# Check the "versions database".
#
# Some repositories use a versions database to maintain version information
# about non-golang dependencies. If found, check it for validity.
check_versions()
{
	local db="versions.yaml"

	[ ! -e "$db" ] && return

	cmd="yamllint"
	[ -n "$(command -v $cmd)" ] && eval "$cmd" "$db"
}

check_commits
check_go
check_versions
