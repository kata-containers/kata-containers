#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

script_dir=$(cd `dirname $0`; pwd)
root_dir=`dirname $script_dir`

test_packages="."

# Set default test run timeout value.
#
# CC_GO_TEST_TIMEOUT can be set to any value accepted by
# "go test -timeout X"
timeout_value=${CC_GO_TEST_TIMEOUT:-10s}

go_test_flags="-v -race -timeout $timeout_value"
cov_file="profile.cov"
tmp_cov_file="profile_tmp.cov"

# Run a command as either root or the current user (which might still be root).
#
# If the first argument is "root", run using sudo, else run as normal.
# All arguments after the first will be treated as the command to run.
function run_as_user
{
	user="$1"
	shift
	cmd=$*

	if [ "$user" = root ]
	then
		# use a shell to ensure PATH is correct.
		sudo -E PATH="$PATH" sh -c "$cmd"
	else
		$cmd
	fi
}

function test_html_coverage
{
	html_report="coverage.html"

	test_coverage

	go tool cover -html="${cov_file}" -o "${html_report}"
	rm -f "${cov_file}"

	run_as_user "current" chmod 644 "${html_report}"
}

function test_coverage
{
	echo "mode: atomic" > "$cov_file"

	if [ $(id -u) -eq 0 ]
	then
		echo >&2 "WARNING: Already running as root so will not re-run tests as non-root user."
		echo >&2 "WARNING: As a result, only a subset of tests will be run"
		echo >&2 "WARNING: (run this script as a non-privileged to ensure all tests are run)."
		users="current"
	else
		# Run the unit-tests *twice* (since some must run as root and
		# others must run as non-root), combining the resulting test
		# coverage files.
		users="current root"
	fi

	for pkg in $test_packages; do
		for user in $users; do
			printf "INFO: Running 'go test' as %s user on packages '%s' with flags '%s'\n" "$user" "$test_packages" "$go_test_flags"

			run_as_user "$user" go test $go_test_flags -covermode=atomic -coverprofile="$tmp_cov_file" $pkg
			if [ -f "${tmp_cov_file}" ]; then
				run_as_user "$user" chmod 644 "$tmp_cov_file"
				tail -n +2 "$tmp_cov_file" >> "$cov_file"
				run_as_user "$user" rm -f "$tmp_cov_file"
			fi
		done
	done
}

function test_local
{
	go test $go_test_flags $test_packages
}

if [ "$1" = "html-coverage" ]; then
	test_html_coverage
elif [ "$CI" = "true" ]; then
	test_coverage
else
	test_local
fi
