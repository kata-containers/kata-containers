#!/bin/bash
#
# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

script_name=${0##*/}
typeset -A long_options

long_options=(
	[help]="Show usage"
	[package:]="Specify test package to run"
)

# Set up go test flags
go_test_flags="${KATA_GO_TEST_FLAGS}"
if [ -z "$go_test_flags" ]; then
    # KATA_GO_TEST_TIMEOUT can be set to any value accepted by
    # "go test -timeout X"
    go_test_flags="-timeout ${KATA_GO_TEST_TIMEOUT:-30s}"

    # -race flag is not supported on s390x
    [ "$(go env GOARCH)" != "s390x" ] && go_test_flags+=" -race"

    # s390x requires special linker flags
    [ "$(go env GOARCH)" = s390x ] && go_test_flags+=" -ldflags '-extldflags -Wl,--s390-pgste'"
fi

# The "master" coverage file that contains the coverage results for
# all packages run under all scenarios.
test_coverage_file="coverage.txt"

# Temporary coverage file created for a "go test" run. The results in
# this file will be added to the master coverage file.
tmp_coverage_file="${test_coverage_file}.tmp"

warn()
{
	local msg="$*"
	echo >&2 "WARNING: $msg"
}

usage()
{
	cat <<EOF

Usage: $script_name [options]

Options:

EOF

	local option
	local description

	local long_option_names="${!long_options[@]}"

	# Sort space-separated list by converting to newline separated list
	# and back again.
	long_option_names=$(echo "$long_option_names"|tr ' ' '\n'|sort|tr '\n' ' ')

	# Display long options
	for option in ${long_option_names}
	do
		description=${long_options[$option]}

		# Remove any trailing colon which is for getopt(1) alone.
		option=$(echo "$option"|sed 's/:$//g')

		printf "    --%-10.10s # %s\n" "$option" "$description"
	done
}

# Run a command as either root or the current user (which might still be root).
#
# If the first argument is "root", run using sudo, else run as the current
# user. All arguments after the first will be treated as the command to run.
run_as_user()
{
	local user="$1"

	shift

	local cmd=$*

	if [ "$user" = root ]; then
		# use a shell to ensure PATH is correct.
		sudo -E PATH="$PATH" sh -c "$cmd"
	else
		eval "$cmd"
	fi
}

# Test a single golang package
test_go_package()
{
	local -r pkg="$1"
	local -r user="$2"

	printf "INFO: Running 'go test' as %s user on package '%s' with flags '%s'\n" \
		"$user" "$pkg" "$go_test_flags"

	run_as_user "$user" go test "$go_test_flags" -covermode=atomic -coverprofile=$tmp_coverage_file "$pkg"

	# Merge test results into the master coverage file.
	run_as_user "$user" tail -n +2 "$tmp_coverage_file" >> "$test_coverage_file"
	rm -f "$tmp_coverage_file"
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
		# Run the unit-tests *twice* (since some must run as
		# root and others must run as non-root), combining the
		# resulting test coverage files.
		users+=" root"
	fi

	echo "INFO: Currently running as user '$(id -un)'"
	for user in $users; do
	    test_go_package "$package" "$user"
	done
}

main()
{
	local long_option_names="${!long_options[@]}"

	local args=$(getopt \
		-n "$script_name" \
		-a \
		--options="h" \
		--longoptions="$long_option_names" \
		-- "$@")

	package="./..."

	eval set -- "$args"
	[ $? -ne 0 ] && { usage >&2; exit 1; }

	while [ $# -gt 1 ]
	do
		case "$1" in
			-h|--help) usage; exit 0 ;;
			--package) package="$2"; shift 2;;
			--) shift; break ;;
		esac

		shift
	done

	test_coverage
}

main "$@"
