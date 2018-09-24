#!/usr/bin/env bash

# Copyright (c) 2017-2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Central script to run all static checks.
#   This script should be called by all other repositories to ensure
#   there is only a single source of all static checks.

set -e

[ -n "$DEBUG" ] && set -x

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

script_name=${0##*/}

repo=""
specific_branch="false"
force="false"

typeset -A long_options

long_options=(
	[commits]="Check commits"
	[docs]="Check document files"
	[files]="Check files"
	[force]="Force a skipped test to run"
	[golang]="Check '.go' files"
	[help]="Display usage statement"
	[licenses]="Check licenses"
	[all]="Force checking of all changes, including files in the base branch"
	[repo:]="Specify GitHub URL of repo to use (github.com/user/repo)"
	[versions]="Check versions files"
)

usage()
{
	cat <<EOT

Usage: $script_name help
       $script_name [options] repo-name [true]

Options:

EOT

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

	cat <<EOT

Parameters:

  help      : Show usage.
  repo-name : GitHub URL of repo to check in form "github.com/user/repo"
              (equivalent to "--repo $URL").
  true      : Specify as "true" if testing the a specific branch, else assume a
              PR branch (equivalent to "--all").

Notes:

- If no options are specified, all non-skipped tests will be run.

Examples:

- Run all tests on a specific branch (stable or master) of runtime repository:

  $ $script_name github.com/kata-containers/runtime true

- Auto-detect repository and run golang tests for current repository:

  $ KATA_DEV_MODE=true $script_name --golang

- Run all tests on the agent repository, forcing the tests to consider all
  files, not just those changed by a PR branch:

  $ $script_name github.com/kata-containers/agent --all


EOT
}

# Convert a golang package to a full path
pkg_to_path()
{
	local pkg="$1"

	go list -f '{{.Dir}}' "$pkg"
}

# Obtain a list of the files the PR changed, ignoring vendor files.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details()
{
	# List of filters used to restrict the types of file changes.
	# See git-diff-tree(1) for further info.
	local filters=""

	# Added file
	filters+="A"

	# Copied file
	filters+="C"

	# Modified file
	filters+="M"

	# Renamed file
	filters+="R"

	# Unmerged (U) and Unknown (X) files. These particular filters
	# shouldn't be necessary but just in case...
	filters+="UX"

	git diff-tree \
		-r \
		--name-status \
		--diff-filter="${filters}" \
		"origin/${target_branch}" HEAD | grep -v "vendor/"
}

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
	if [ ! "$(command -v $linter)" ]
	then
		info "Installing ${linter}"

		local linter_url="github.com/alecthomas/gometalinter"
		go get -d "$linter_url"

		# Pin to known good version.
		#
		# This project changes a lot but we don't want newly-added
		# linter checks to break valid PR code.
		#
		local linter_version=$(get_version "externals.gometalinter.version")

		info "Forcing ${linter} version ${linter_version}"

		(cd "$GOPATH/src/$linter_url" && git checkout "$linter_version" && go install)
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

	if [ "$(uname -s)" == "Linux" ]; then
		linter_args+=" --concurrency=$(nproc)"
	elif [ "$(uname -s)" == "Darwin" ]; then
		linter_args+=" --concurrency=$(sysctl -n hw.activecpu)"
	fi

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

	info "$linter args: '$linter_args'"

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

	info "Running $linter checks on the following packages:\n"
	echo "$go_packages"
	echo
	info "Package paths:\n"
	echo "$dirs" | sed 's/^ *//g' | tr ' ' '\n'

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
	if [ -n "$(command -v $cmd)" ]; then
		eval "$cmd" "$db"
	fi
}

# Ensure all files (where possible) contain an SPDX license header
check_license_headers()
{
	# The branch is the baseline - ignore it.
	[ "$specific_branch" = "true" ] && return

	# See: https://spdx.org/licenses/Apache-2.0.html
	local -r spdx_tag="SPDX-License-Identifier"
	local -r spdx_license="Apache-2.0"
	local -r pattern="${spdx_tag}: ${spdx_license}"

	info "Checking for SPDX license headers"

	files=$(get_pr_changed_file_details || true)

	# Strip off status
	files=$(echo "$files"|awk '{print $NF}')

	# no files were changed
	[ -z "$files" ] && info "No files found" && return

	local missing=$(egrep \
		--exclude=".git/*" \
		--exclude=".gitignore" \
		--exclude="Gopkg.lock" \
		--exclude="LICENSE" \
		--exclude="protocols/grpc/*.pb.go" \
		--exclude="vendor/*" \
		--exclude="VERSION" \
		--exclude="*.jpg" \
		--exclude="*.json" \
		--exclude="*.md" \
		--exclude="*.png" \
		--exclude="*.toml" \
		--exclude="*.yaml" \
		-EL "\<${pattern}\>" \
		$files || true)

	if [ -n "$missing" ]; then
		cat >&2 <<-EOT
		ERROR: Required license identifier ('$pattern') missing from following files:

		$missing

EOT
		exit 1
	fi
}

# Perform basic checks on documentation files
check_docs()
{
	local cmd="xurls"

	if [ ! "$(command -v $cmd)" ]
	then
		info "Installing $cmd utility"
		go get -u "mvdan.cc/xurls/cmd/$cmd"
	fi

	info "Checking documentation"

	local doc
	local docs
	local docs_status
	local new_docs
	local new_urls
	local url

	if [ "$specific_branch" = "true" ]
	then
		info "Checking all documents in $branch branch"

		docs=$(find . -name "*.md" | grep -v "vendor/" || true)
	else
		info "Checking local branch for changed documents only"

		docs_status=$(get_pr_changed_file_details || true)
		docs_status=$(echo "$docs_status" | grep "\.md$" || true)

		docs=$(echo "$docs_status" | awk '{print $NF}')

		# Newly-added docs
		new_docs=$(echo "$docs_status" | awk '/^A/ {print $NF}')

		for doc in $new_docs
		do
			# A new document file has been added. If that new doc
			# file is referenced by any files on this PR, checking
			# its URL will fail since the PR hasn't been merged
			# yet. We could construct the URL based on the users
			# original PR branch and validate that. But it's
			# simpler to just construct the URL that the "pending
			# document" *will* result in when the PR has landed
			# and then check docs for that new URL and exclude
			# them from the real URL check.
			url="https://${repo}/blob/${target_branch}/${doc}"

			new_urls+=" ${url}"
		done
	fi

	[ -z "$docs" ] && info "No documentation to check" && return

	local urls
	local url_map=$(mktemp)
	local invalid_urls=$(mktemp)

	info "Checking document code blocks"

	for doc in $docs
	do
		bash "${cidir}/kata-doc-to-script.sh" -csv "$doc"

		# Look for URLs in the document
		urls=$($cmd "$doc")

		# Gather URLs
		for url in $urls
		do
			printf "%s\t%s\n" "${url}" "${doc}" >> "$url_map"
		done
	done

	# Get unique list of URLs
	urls=$(awk '{print $1}' "$url_map" | sort -u)

	info "Checking all document URLs"

	for url in $urls
	do
		if [ "$specific_branch" != "true" ]
		then
			# If the URL is new on this PR, it cannot be checked.
			echo "$new_urls" | grep -q "\<${url}\>" && \
				info "ignoring new (but correct) URL: $url" && continue
		fi

		# Ignore the install guide URLs that contain a shell variable
		echo "$url" | grep -q "\\$" && continue

		# This prefix requires the client to be logged in to github, so ignore
		echo "$url" | grep -q 'https://github.com/pulls' && continue

		# Sigh.
		echo "$url"|grep -q 'https://example.com' && continue
		
		# Google APIs typically require an auth token.
		echo "$url"|grep -q 'https://www.googleapis.com' && continue

		# Check the URL, saving it if invalid
		( curl -sLf -o /dev/null "$url" ||\
				echo "$url" >> "$invalid_urls") &
	done

	# Synchronisation point
	wait

	if [ -s "$invalid_urls" ]
	then
		local files

		cat "$invalid_urls" | while read url
		do
			files=$(grep "^${url}" "$url_map" | awk '{print $2}' | sort -u)
			echo >&2 -e "ERROR: Invalid URL '$url' found in the following files:\n"

			for file in $files
			do
				echo >&2 "$file"
			done
		done

		exit 1
	fi

	rm -f "$url_map" "$invalid_urls"
}

# Tests to apply to all files.
#
# Currently just looks for TODO/FIXME comments that should be converted to
# (or annotated with) an Issue URL.
check_files()
{
	local file
	local files

	if [ "$force" = "false" ]
	then
		info "Skipping check_files: see https://github.com/kata-containers/tests/issues/469"
		return
	else
		info "Force override of check_files skip"
	fi

	info "Checking files"

	if [ "$specifc_branch" = "true" ]
	then
		info "Checking all files in $branch branch"

		files=$(find . -type f | egrep -v "(.git|vendor)/" || true)
	else
		info "Checking local branch for changed files only"

		files=$(get_pr_changed_file_details || true)

		# Strip off status
		files=$(echo "$files"|awk '{print $NF}')
	fi

	[ -z "$files" ] && info "No files changed" && return

	local matches=""

	for file in $files
	do
		local match

		# Look for files containing the specified comment tags but
		# which do not include a github URL.
		match=$(egrep -H "\<FIXME\>|\<TODO\>" "$file" |\
			grep -v "https://github.com/.*/issues/[0-9]" |\
			cut -d: -f1 |\
			sort -u || true)

		[ -z "$match" ] && continue

		# Don't fail if this script contains the patterns
		# (as it is guaranteed to ;)
		echo "$file" | grep -q "${script_name}$" && info "Ignoring special file $file" && continue

		# We really only care about comments in code. But to avoid
		# having to hard-code the list of file extensions to search,
		# invert the problem by simply ignoring document files and
		# considering all other file types.
		echo "$file" | grep -q ".md$" && info "Ignoring comment tag in document $file" && continue

		matches+=" $match"
	done

	[ -z "$matches" ] && return

	echo >&2 -n \
		"ERROR: The following files contain TODO/FIXME's that need "
	echo >&2 -e "converting to issues:\n"

	for file in $matches
	do
		echo >&2 "$file"
	done

	# spacer
	echo >&2

	exit 1
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

	eval set -- "$args"
	[ $? -ne 0 ] && { usage >&2; exit 1; }

	local func=

	while [ $# -gt 1 ]
	do
		case "$1" in
			--commits) func=check_commits ;;
			--docs) func=check_docs ;;
			--files) func=check_files ;;
			--force) force="true" ;;
			--golang) func=check_go ;;
			-h|--help) usage; exit 0 ;;
			--licenses) func=check_license_headers ;;
			--all) specific_branch="true" ;;
			--repo) repo="$2"; shift ;;
			--versions) func=check_versions ;;
			--) shift; break ;;
		esac

		shift
	done

	# Consume getopt cruft
	[ "$1" = "--" ] && shift

	[ "$1" = "help" ] && usage && exit 0

	# Set if not already set by options
	[ -z "$repo" ] && repo="$1"
	[ "$specific_branch" = "false" ] && specific_branch="$2"


	if [ -z "$repo" ]
	then
		if [ -n "$KATA_DEV_MODE" ]
		then
			# No repo param provided so assume it's the current
			# one to avoid developers having to specify one now
			# (backwards compatability).
			repo=$(git config --get remote.origin.url |\
				sed 's!https://!!g' || true)

			info "Auto-detected repo as $repo"
		else
			echo >&2 "ERROR: need repo" && usage && exit 1
		fi
	fi

	# Run user-specified check and quit
	[ -n "$func" ] && info "running $func function" && eval "$func" && exit 0

	# Run all checks
	if [ -n "$TRAVIS_BRANCH" ] && [ "$TRAVIS_BRANCH" != "master" ]
	then
		echo "Skipping checkcommits"
		echo "See issue: https://github.com/kata-containers/tests/issues/632"
	else
		check_commits
	fi
	check_license_headers
	check_go
	check_versions
	check_docs
	check_files
}

main "$@"
