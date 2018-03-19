#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

# Set default test run timeout value.
#
# KATA_GO_TEST_TIMEOUT can be set to any value accepted by
# "go test -timeout X"
timeout_value=${KATA_GO_TEST_TIMEOUT:-10s}

# -race flag is supported only on amd64/x86_64 arch , hence 
# enabling the flag depending on the arch.
[ "$(go env GOARCH)" = "amd64" ] && race="-race"

# KATA_GO_TEST_FLAGS can be set to change the flags passed to "go test".
go_test_flags=${KATA_GO_TEST_FLAGS:-"-v $race -timeout $timeout_value"}

# Note: the vendor filtering is required for versions of go older than 1.9
test_packages=$(go list ./... 2>/dev/null | grep -v "/vendor/" || true)

# The "master" coverage file that contains the coverage results for
# all packages run under all scenarios.
test_coverage_file="profile.cov"

# Temporary coverage file created for a single package. The results in this
# file will be added to the master coverage file.
tmp_coverage_file="profile-tmp.cov"

# Permissions to create coverage files with
coverage_file_mode=0644

# Name of HTML format coverage file
html_report_file="coverage.html"

warn()
{
	local msg="$*"
	echo >&2 "WARNING: $msg"
}

# Run a command as either root or the current user (which might still be root).
#
# If the first argument is "root", run using sudo, else run as the current
# user. All arguments after the first will be treated as the command to run.
run_as_user()
{
	user="$1"
	shift
	cmd=$*

	if [ "$user" = root ]; then
		# use a shell to ensure PATH is correct.
		sudo -E PATH="$PATH" sh -c "$cmd"
	else
		$cmd
	fi
}

# Run the tests and generate an HTML report of the results
test_html_coverage()
{
	test_coverage

	go tool cover -html="${test_coverage_file}" -o "${html_report_file}"
	rm -f "${test_coverage_file}"

	run_as_user "current" chmod "${coverage_file_mode}" "${html_report_file}"
}

# Run all tests and generate a test coverage file.
test_coverage()
{
	echo "mode: atomic" > "$test_coverage_file"

	users="current"

	if [ "$(id -u)" -eq 0 ]; then
		warn "Already running as root so will not re-run tests as non-root user."
		warn "As a result, only a subset of tests will be run"
		warn "(run this script as a non-privileged to ensure all tests are run)."
	else
		if [ -n "$KATA_DEV_MODE" ]; then
			warn "Dangerous to set CI and KATA_DEV_MODE together."
			warn "NOT running tests as root."
		else
			# Run the unit-tests *twice* (since some must run as root and
			# others must run as non-root), combining the resulting test
			# coverage files.
			users+=" root"
		fi
	fi

	echo "INFO: Currently running as user '$(id -un)'"

	for pkg in $test_packages; do
		for user in $users; do
			printf "INFO: Running 'go test' as %s user on package '%s' with flags '%s'\n" \
				"$user" "$pkg" "$go_test_flags"

			eval run_as_user "$user" go test "$go_test_flags" -covermode=atomic -coverprofile="$tmp_coverage_file" "$pkg"

			# Check for the temporary coverage file since if will
			# not be generated unless a package actually contains
			# tests.
			if [ -f "${tmp_coverage_file}" ]; then
				# Save these package test results into the
				# master coverage file.
				run_as_user "$user" chmod "${coverage_file_mode}" "$tmp_coverage_file"
				tail -n +2 "$tmp_coverage_file" >> "$test_coverage_file"
				run_as_user "$user" rm -f "$tmp_coverage_file"
			fi
		done
	done
}

# Run the tests locally
test_local()
{
	for pkg in $test_packages; do
		eval go test "$go_test_flags" "$pkg"
	done
}

main()
{
	[ -z "$test_packages" ] && echo "INFO: no golang code to test" && exit 0

	if [ "$1" = "html-coverage" ]; then
		test_html_coverage
	elif [ "$CI" = "true" ]; then
		test_coverage
	else
		test_local
	fi
}

main "$@"
