#!/usr/bin/env bash

# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Perform CI 'fast path return' checks.
# Returns: 0 for 'fast return OK', 1 for 'do not fastpath'.
#
# Checks for:
#  - file name patterns from the YAML that will force the CI to run
#  - file name patterns from the YAML to see if we can skip all files in the PR
#  - If potentially skipping, looks for a 'force' label on the PR to force the CI
#    to run anyway

set -e

[ -n "$DEBUG" ] && set -x

script_name="${0##*/}"
script_dir_base=".ci"
script_gitname="${script_dir_base}/${script_name}"

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

# If no branch specified, compare against the master.
# The 'branch' var is required by the get_pr() lib functions.
branch=${branch:-master}

# The YAML file containing our filename match patterns.
yqfile_rootname="ci-fast-return.yaml"
yqfile="${cidir}/${yqfile_rootname}"
yqfile_gitname="${script_dir_base}/${yqfile_rootname}"

# The YAML file containing our unit test patterns
unit_yamlfile_rootname="ci-fast-return/test.yaml"
unit_yamlfile="${cidir}/${unit_yamlfile_rootname}"
unit_yamlfile_gitname="${script_dir_base}/${unit_yamlfile_rootname}"

# Name of the label, that if set on a PR, will force the CI to be run anyway.
force_label="force-ci"

# The list of files to check is held in a global, to make writing the unit test
# code easier - otherwise we would have to pass two multi-entry lists (the list
# of expressions and the list of filenames) to a single test function, which is
# tricky.
filenames=""

# We have a local info func, as many of our funcs return their answers via stdout,
# and we don't want to either open code a redirect all over the code nor send to
# the standard stdout (would corrupt our return strings) or stderr (some CIs would
# take that as a failure case). So, use another file descriptor, mapped back to
# stdout, that we set up previously.
local_info() {
	msg="$*"
	info "$msg" >&5
}

# Read our patterns from the YAML file.
# Arguments:
# $1 - the yaml file path
# $2 - the yaml (yq) path to the patterns
#
# Returns on stdout
#  the patterns found, or "" if no patterns (it will translate the yq 'null' return
#  to the empty string)
#
read_yaml() {
	${cidir}/install_yq.sh >&2

	res=$(yq read "$1" "$2")
	[ "$res" == "null" ] && res=""
	echo $res
	return 0
}

# Check if any files in ${filenames} match the egrep command line expressions
# passed in as arguments. Note, the arguments must be formatted *as passed*
# to egrep. Thus, a single expression can just be passed on its own, but multiple
# expressions must individually be prefixed with '-e' or '--regexp='.
# Returns on stdout any files that match the patterns.
check_matches() {
	egrep $@ <<< "$filenames" || true
}

# Check if any files in ${filenames} *DO NOT* match the egrep command line expressions
# passed in as arguments. Note, the arguments must be formatted *as passed*
# to egrep. Thus, a single expression can just be passed on its own, but multiple
# expressions must individually be prefixed with '-e' or '--regexp='.
# Returns on stdout any files that *DO NOT* match the patterns.
check_not_matches() {
	egrep -v $@ <<< "$filenames" || true
}

# Check if any files changed by the PR either force the CI to run or if
# we can skip all the files in the PR.
#
# Returns stdout string:
#  0 - all files are fastpath - yes, we can skip.
#  1 - at least one non-fastpath file found - cannot skip
can_we_skip() {
	# The branch is the baseline - ignore it.
	if [ "$specific_branch" = "true" ]; then
		local_info "Skip baseline branch"
		echo "0"
		return 0
	fi

	filenames=$(get_pr_changed_file_details || true)
	# Strip off the leading status - just grab last column.
	filenames=$(echo "$filenames"|awk '{print $NF}')

	# no files were changed - I guess we can skip the CI then?
	if [ -z "$filenames" ]; then
		local_info "No files found"
		echo "0"
		return 0
	fi

	# Check to see if this file or any of its deps changed, as then we should
	# run our own internal unit tests.
	check_for_self_test

	# Get our common patterns we check against all repos.
	local common_skip=$(read_yaml ${yqfile} common.skip_patterns)
	local common_check=$(read_yaml ${yqfile} common.check_patterns)

	# Use just the repo name itself. The YAML does not like having the full
	# repo path in it.
	local repo="${kata_repo##*/}"

	if [ -n "$repo" ]; then
		# Get our repo specific patterns
		local repo_skip=$(read_yaml ${yqfile} ${repo}.skip_patterns)
		local repo_check=$(read_yaml ${yqfile} ${repo}.check_patterns)
	else
		local_info "No repo set, skipping repo specific patterns"
	fi

	local canskip_exprs=""
	for x in $common_skip $repo_skip; do
		# Build up the string of "-e exp1 -e exp2" arguments to later pass
		# to the egrep based search.
		canskip_exprs+=$(echo "-e $x ")
	done

	local mustcheck_exprs=""
	for x in $common_check $repo_check; do
		# Build up the string of "-e exp1 -e exp2" arguments to later pass
		# to the egrep based search.
		mustcheck_exprs+=$(echo "-e $x ")
	done

	local_info "Skip patterns: [$canskip_exprs]"
	local_info "Check patterns: [$mustcheck_exprs]"

	# do we have any patterns to check?
	if [ -n "$mustcheck_exprs" ]; then
		local need_checking=$(check_matches ${mustcheck_exprs[@]})
	else
		local_info "No force CI check patterns"
		local need_checking=""
	fi

	# If we have any files that *must* be checked, then immediately return
	if [ -n "$need_checking" ]; then
		# stderr, so it does not get in our return value...
		cat >&2 <<-EOT
		INFO: Cannot fastpath skip CI.
		INFO: Some files present must be CI checked.
		INFO: Files to check are:

		$need_checking

EOT
		echo "1"
		return 0
	else
		local_info "No force check files found"
	fi

	# Now we have checked there are no 'must check' files, we can check to see
	# if all files fall into the 'can skip' patterns.
	# first, do we have any skip patterns to search for?
	if [ -n "$canskip_exprs" ]; then
		local non_skippable=$(check_not_matches ${canskip_exprs[@]} <<< "$filenames")
	else
		local_info "No skip CI check patterns"
		# No patterns, set non-skippable list to all files then...
		local non_skippable="$filenames"
	fi

	if [ -z "$non_skippable" ]; then
		# stderr, so it does not get in our return value...
		cat >&2 <<-EOT
		INFO: No files to check in CI.
		INFO: Fastpath short circuit returning from CI.
		INFO: Files skipped are:

		$filenames

EOT
		echo "0"
		return 0
	else
		cat >&2 <<-EOT
		INFO: Not all files skippable

		$non_skippable

EOT
		echo "1"
		return 0
	fi
}

# Check if we have the 'magic label' that forces a CI run set on the PR
# Returns on stdout as string:
#  0 - No label found, could skip the CI
#  1 - Label found - should run the CI
check_force_label() {
	if [ -z "$ghprbGhRepository" ]; then
		local_info "No ghprbGhRepository set, skip label check"
		echo "0"
		return 0
	fi

	if [ -z "$ghprbPullId" ]; then
		local_info "No ghprbPullId set, skip label check"
		echo "0"
		return 0
	fi

	local_info "Checking labels for PR ${ghprbGhRepository}/${ghprbPullId}"

	# Pull the label list for the PR
	# Ideally we'd use a github auth token here so we don't get rate limited, but to do that we would
	# have to expose the token into the CI scripts, which is then potentially a security hole.
	local json=$(curl -sL https://api.github.com/repos/${ghprbGhRepository}/issues/${ghprbPullId}/labels)

	# Pull the label list out
	local labels=$(jq .[].name <<< $json)

	# Check if we have the forcing label set
	for x in $labels; do
		# Strip off any surrounding '"'s
		y=$(sed 's/"//g' <<< $x)

		if [ "$y" == "$force_label" ]; then
			local_info "label found, forcing CI"
			echo "1"
			return 0
		fi
	done

	local_info "No forcing label found"
	echo "0"
	return 0
}

# Unit tests. Check the YAML file reader functions work as expected.
testYAMLreader() {
	repos=()
	patterns=()
	results=()

	# Check we can read 'common' patterns, and they act as we expect.
	repos+=("common")
	patterns+=("skip_patterns")
	results+=("^CODEOWNERS$ .*\.md")

	repos+=("common")
	patterns+=("check_patterns")
	results+=("check1 .*check2$")

	# Check a pattern with no list does not fault.
	repos+=("common")
	patterns+=("empty_patterns")
	results+=("")

	# Check we do not fault looking for a pattern that is not defined.
	repos+=("common")
	patterns+=("undefined_patterns")
	results+=("")

	# Check we can read a named repo.
	# Check single line expression entries work.
	repos+=("documentation")
	patterns+=("doc_single_entry")
	results+=("single_pattern")

	# Check we can handle multiple complex regexps.
	repos+=("documentation")
	patterns+=("doc_complex_patterns")
	results+=("^[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}$ ^#?([a-f0-9]{6}|[a-f0-9]{3})$")

	# Check we do not fail if trying to assess a repo that is not defined.
	repos+=("nonrepo")
	patterns+=("nonrepo_pattern")
	results+=("")

	count=0
	for x in ${repos[@]}; do
		echo " $count: Testing $x:${patterns[$count]} == '${results[$count]}'"

		res=$(read_yaml "$unit_yamlfile" "${x}.${patterns[$count]}")
		assertEquals "${results[$count]}" "$res"

		count=$(( count+1 ))
	done
}

# Unit test: Check the pattern matcher functions work as expected.
testPatternMatcher() {
	filenames="fred
john
mark
doc.md"

	matches=$(check_matches '.*\.md')
	assertEquals "doc.md" "$matches"

	# Check it also works with `-e` multiple expressions
	matches=$(check_matches '-e notaname -e .*\.md')
	assertEquals "doc.md" "$matches"

	matches=$(check_matches 'mark')
	assertEquals "mark" "$matches"

	matches=$(check_matches '^.*$')
	assertEquals "$filenames" "$matches"

	matches=$(check_not_matches '.*\.md')
	assertEquals "fred
john
mark" "$matches"

	# check with multi patterns
	matches=$(check_not_matches '-e .*\.md -e john')
	assertEquals "fred
mark" "$matches"


	matches=$(check_not_matches 'mark')
	assertEquals "fred
john
doc.md" "$matches"

	matches=$(check_not_matches '^.*$')
	assertEquals "" "$matches"

}

# Check if any of our own files have changed in this PR, and if so, run our own
# unit tests...
check_for_self_test() {
	local ourfiles=""

	ourfiles+="-e ^${script_gitname}$ "
	ourfiles+="-e ^${yqfile_gitname}$ "
	ourfiles+="-e ^${unit_yamlfile_gitname}$ "

	res=$(check_matches $ourfiles)

	if [ -n "$res" ]; then
		local_info "file(s) [$res] modified - self running unit tests"
		# Need to push the unit test output via the stdout mapped file descriptor
		# so we don't corrupt our actual test function return values.
		${cidir}/${script_name} test >&5
	fi
}

# Run our self tests. Tests are written using the
# github.com/kward/shunit2 library, and are encoded into functions starting
# with the string 'test'.
self_test() {
	local shunit2_path="github.com/kward/shunit2"
	local_info "Running self tests"

	local_info "Go get unit test framework from ${shunit2_path}"
	go get -d "${shunit2_path}" || true
	local_info "Run the unit tests"
	# Sourcing the `shunit2` file automatically runs the unit tests in this file.
	. "${GOPATH}/src/${shunit2_path}/shunit2"
	# shunit2 call does not return - it exits with its return code.
}

help()
{
	cat <<EOT
Usage: ${script_name} [test]

This script will check if the CI system needs to be run, according
to the egrep patterns found in the file ${yqfile}

Passing the argument 'test' to this script will cause it to only
run its self tests.
EOT

	exit 0
}

main() {

	# Some of our sub-funcs return their results on stdout, but we also want them to be
	# able to log INFO messages. But, we don't want those going to stderr, as that may
	# be seen by some CIs as an actual error. Create another file descriptor, mapped
	# back to stdout, for us to send INFO messages to...
	exec 5>&1

	if [ "$1" == "test" ]; then
		self_test
		# self_test func does not return
	fi

	[ $# -gt 0 ] && help

	info "Checking for any changed files that will prevent CI fastpath return"
	res=$(can_we_skip)

	# If the file check says we can skip, check the labels to see if we are forcing the
	# CI run anyway.
	[ $res -eq 0 ] && res=$(check_force_label)
	exit $res
}

main "$@"
