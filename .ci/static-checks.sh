#!/usr/bin/env bash

# Copyright (c) 2017-2019 Intel Corporation
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

export tests_repo="${tests_repo:-github.com/kata-containers/tests}"
export tests_repo_dir="${GOPATH}/src/${tests_repo}"

# List of files to delete on exit
files_to_remove=()

script_name=${0##*/}

# Static check functions must follow the following naming conventions:
#

# All static check function names must match this pattern.
typeset -r check_func_regex="^static_check_"

# All architecture-specific static check functions must match this pattern.
typeset -r arch_func_regex="_arch_specific$"

repo=""
specific_branch="false"
force="false"
branch=${branch:-master}

# Which static check functions to consider.
handle_funcs="all"

single_func_only="false"
list_only="false"

# number of seconds to wait for curl to check a URL
typeset url_check_timeout_secs="${url_check_timeout_secs:-60}"

# number of attempts that will be made to check an individual URL.
typeset url_check_max_tries="${url_check_max_tries:-3}"

typeset -A long_options

long_options=(
	[commits]="Check commits"
	[docs]="Check document files"
	[files]="Check files"
	[force]="Force a skipped test to run"
	[golang]="Check '.go' files"
	[help]="Display usage statement"
	[labels]="Check labels databases"
	[licenses]="Check licenses"
	[list]="List tests that would run"
	[branch]="Specify upstream branch to compare against (default '$branch')"
	[all]="Force checking of all changes, including files in the base branch"
	[repo:]="Specify GitHub URL of repo to use (github.com/user/repo)"
	[vendor]="Check vendor files"
	[versions]="Check versions files"
	[no-arch]="Run/list all tests except architecture-specific ones"
	[only-arch]="Only run/list architecture-specific tests"
)

yamllint_cmd="yamllint"
have_yamllint_cmd=$(command -v "$yamllint_cmd" || true)

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
              (equivalent to "--repo \$URL").
  true      : Specify as "true" if testing a specific branch, else assume a
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

# Calls die() if the specified function is not valid.
func_is_valid() {
    local name="$1"

    type -t "$name" &>/dev/null || die "function '$name' does not exist"
}

# Calls die() if the specified function is not valid or not a check function.
ensure_func_is_check_func() {
    local name="$1"

    func_is_valid "$name"

    { echo "$name" | grep -q "${check_func_regex}"; ret=$?; }

    [ "$ret" = 0 ] || die "function '$name' is not a check function"
}

# Returns "yes" if the specified function needs to run on all architectures,
# else "no".
func_is_arch_specific() {
    local name="$1"

    ensure_func_is_check_func "$name"

    { echo "$name" | grep -q "${arch_func_regex}"; ret=$?; }

    if [ "$ret" = 0 ]; then
        echo "yes"
    else
        echo "no"
    fi
}

function remove_tmp_files() {
	rm -rf "${files_to_remove[@]}"
}

# Convert a golang package to a full path
pkg_to_path()
{
	local pkg="$1"

	go list -f '{{.Dir}}' "$pkg"
}

# Obtain a list of the files the PR changed.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details_full()
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
		"origin/${branch}" HEAD
}

# Obtain a list of the files the PR changed, ignoring vendor files.
# Returns the information in format "${filter}\t${file}".
get_pr_changed_file_details()
{
	get_pr_changed_file_details_full | grep -v "vendor/"
}

static_check_commits()
{
	# Since this script is called from another repositories directory,
	# ensure the utility is built before running it.
	(cd "${tests_repo_dir}" && make checkcommits)

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

static_check_go_arch_specific()
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
	git submodule -q foreach "go list ./..." | sort > "$submodule_packages" || true

	# all packages
	all_packages=$(mktemp)
	go list ./... | sort > "$all_packages" || true

	files_to_remove+=("$submodule_packages" "$all_packages")

	# List of packages to consider which is defined as:
	#
	#   "all packages" - "submodule packages"
	#
	# Note: the vendor filtering is required for versions of go older than 1.9
	go_packages=$(comm -3 "$all_packages" "$submodule_packages" | grep -v "/vendor/" || true)

	# No packages to test
	[ -z "$go_packages" ] && return

	local linter="golangci-lint"

	# Run golang checks
	if [ ! "$(command -v $linter)" ]
	then
		info "Installing ${linter}"

		local linter_url=$(get_test_version "externals.golangci-lint.url")
		local linter_version=$(get_test_version "externals.golangci-lint.version")

		info "Forcing ${linter} version ${linter_version}"
		build_version ${linter_url} "build" ${linter_version}
		# golangci build only target does not install into ${GOPATH}/bin
		# Hand install it...
		go install -v "${linter_url}/..."
	fi

	local linter_args="run -c ${cidir}/.golangci.yml"

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
static_check_versions()
{
	local db="versions.yaml"

	[ ! -e "$db" ] && return

	if [ -n "$have_yamllint_cmd" ]; then
		eval "$yamllint_cmd" "$db"
	else
		info "Cannot check versions as $yamllint_cmd not available"
	fi
}

static_check_labels()
{
	[ $(uname -s) != Linux ] && info "Can only check labels under Linux" && return

	# Handle SLES which doesn't provide the required command.
	[ -z "$have_yamllint_cmd" ] && info "Cannot check labels as $yamllint_cmd not available" && return

	# Since this script is called from another repositories directory,
	# ensure the utility is built before the script below (which uses it) is run.
	(cd "${tests_repo_dir}" && make github-labels)

	tmp=$(mktemp)

	files_to_remove+=("${tmp}")

	info "Checking labels for repo ${repo} using temporary combined database ${tmp}"

	bash -f "${tests_repo_dir}/cmd/github-labels/github-labels.sh" "generate" "${repo}" "${tmp}"
}

# Ensure all files (where possible) contain an SPDX license header
static_check_license_headers()
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
		--exclude="vendor/*" \
		--exclude="VERSION" \
		--exclude="*.jpg" \
		--exclude="*.json" \
		--exclude="*.md" \
		--exclude="*.png" \
		--exclude="*.pub" \
		--exclude="*.service" \
		--exclude="*.svg" \
		--exclude="*.toml" \
		--exclude="*.txt" \
		--exclude="*.yaml" \
		--exclude="*.pb.go" \
		--exclude="*.gpl.c" \
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

check_url()
{
	local url="$1"
	local invalid_urls_dir="$2"

	local curl_out=$(mktemp)
	files_to_remove+=("${curl_out}")

	info "Checking URL $url"

	# Process specific file to avoid out-of-order writes
	local invalid_file=$(printf "%s/%d" "$invalid_urls_dir" "$$")

	local ret

	{ curl -sIL --max-time "$url_check_timeout_secs" --retry "$url_check_max_tries" "$url" &>"$curl_out"; ret=$?; } || true

	# A transitory error, or the URL is incorrect,
	# but capture either way.
	if [ "$ret" -ne 0 ]; then
		echo "$url" >> "${invalid_file}"

		die "check failed for URL $url after $url_check_max_tries tries"
	fi

	local http_statuses

	http_statuses=$(grep -E "^HTTP" "$curl_out" | awk '{print $2}' || true)
	if [ -z "$http_statuses" ]; then
		echo "$url" >> "${invalid_file}"
		die "no HTTP status codes for URL $url"
	fi

	local status

	for status in $http_statuses
	do
		# Ignore the following ranges of status codes:
		#
		# - 1xx: Informational codes.
		# - 2xx: Success codes.
		# - 3xx: Redirection codes.
		# - 405: Specifically to handle some sites
		#   which get upset by "curl -L" when the
		#   redirection is not required.
		#
		# Anything else is considered an error.
		#
		# See https://en.wikipedia.org/wiki/List_of_HTTP_status_codes

		if ! echo "$status" | grep -qE "^(1[0-9][0-9]|2[0-9][0-9]|3[0-9][0-9]|405)"; then
			echo "$url" >> "$invalid_file"
			die "found HTTP error status codes for URL $url"
		fi
	done
}

# Perform basic checks on documentation files
static_check_docs()
{
	local cmd="xurls"

	if [ ! "$(command -v $cmd)" ]
	then
		info "Installing $cmd utility"

		local version
		local url
		local dir

		version=$(get_test_version "externals.xurls.version")
		url=$(get_test_version "externals.xurls.url")
		dir=$(echo "$url"|sed 's!https://!!g')

		build_version "${dir}" "" "${version}"
	fi

	info "Checking documentation"

	local doc
	local all_docs
	local docs
	local docs_status
	local new_docs
	local new_urls
	local url

	all_docs=$(find . -name "*.md" | grep -v "vendor/" | sort || true)

	if [ "$specific_branch" = "true" ]
	then
		info "Checking all documents in $branch branch"
		docs="$all_docs"
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
			url="https://${repo}/blob/${branch}/${doc}"

			new_urls+=" ${url}"
		done
	fi

	[ -z "$docs" ] && info "No documentation to check" && return

	local urls
	local url_map=$(mktemp)
	local invalid_urls=$(mktemp)
	local md_links=$(mktemp)
	files_to_remove+=("${url_map}" "${invalid_urls}" "${md_links}")

	info "Checking document markdown references"

	local md_docs_to_check

	# All markdown docs are checked (not just those changed by a PR). This
	# is necessary to guarantee that all docs are referenced.
	md_docs_to_check="$all_docs"

	(cd "${tests_repo_dir}" && make check-markdown)

	for doc in $md_docs_to_check
	do
		kata-check-markdown check "$doc"

		# Get a link of all other markdown files this doc references
		kata-check-markdown list links --format tsv --no-header "$doc" |\
			grep "external-link" |\
			awk '{print $3}' |\
			sort -u >> "$md_links"
	done

	# clean the list of links
	local tmp
	tmp=$(mktemp)

	sort -u "$md_links" > "$tmp"
	mv "$tmp" "$md_links"

	# Remove initial "./" added by find(1).
	md_docs_to_check=$(echo "$md_docs_to_check"|sed 's,^\./,,g')

	# A list of markdown files that do not have to be referenced by any
	# other markdown file.
	exclude_doc_regexs+=()

	exclude_doc_regexs+=(^CODE_OF_CONDUCT\.md$)
	exclude_doc_regexs+=(^CONTRIBUTING\.md$)

	# Magic github template files
	exclude_doc_regexs+=(^\.github/.*\.md$)

	# The top level README doesn't need to be referenced by any other
	# since it displayed by default when visiting the repo.
	exclude_doc_regexs+=(^README\.md$)

	local exclude_pattern

	# Convert the list of files into an egrep(1) alternation pattern.
	exclude_pattern=$(echo "${exclude_doc_regexs[@]}"|sed 's, ,|,g')

	# Every document in the repo (except a small handful of exceptions)
	# should be referenced by another document.
	for doc in $md_docs_to_check
	do
		# Check the ignore list for markdown files that do not need to
		# be referenced by others.
		echo "$doc"|egrep -q "(${exclude_pattern})" && continue

		grep -q "$doc" "$md_links" || die "Document $doc is not referenced"
	done

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
	local invalid_urls_dir=$(mktemp -d)
	files_to_remove+=("${invalid_urls_dir}")

	for url in $urls
	do
		if [ "$specific_branch" != "true" ]
		then
			# If the URL is new on this PR, it cannot be checked.
			echo "$new_urls" | egrep -q "\<${url}\>" && \
				info "ignoring new (but correct) URL: $url" && continue
		fi

		# Ignore local URLs. The only time these are used is in
		# examples (meaning these URLs won't exist).
		echo "$url" | grep -q "^file://" && continue

		# Ignore the install guide URLs that contain a shell variable
		echo "$url" | grep -q "\\$" && continue

		# This prefix requires the client to be logged in to github, so ignore
		echo "$url" | grep -q 'https://github.com/pulls' && continue

		# Sigh.
		echo "$url"|grep -q 'https://example.com' && continue
		
		# Google APIs typically require an auth token.
		echo "$url"|grep -q 'https://www.googleapis.com' && continue

		# Git repo URL check
		if echo "$url"|grep -q '^https.*git'
		then
			timeout "${KATA_NET_TIMEOUT}" git ls-remote "$url" > /dev/null 2>&1 && continue
		fi

		# Check the URL, saving it if invalid
		#
		# Each URL is checked in a separate process as each unique URL
		# requires us to hit the network.
		check_url "$url" "$invalid_urls_dir" &
	done

	# Synchronisation point
	wait

	# Combine all the separate invalid URL files into one
	local invalid_files=$(ls "$invalid_urls_dir")

	if [ -n "$invalid_files" ]; then
		pushd "$invalid_urls_dir" &>/dev/null
		cat $(echo "$invalid_files"|tr '\n' ' ') > "$invalid_urls"
		popd &>/dev/null
	fi

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

	# Now, spell check the docs
	cmd="${tests_repo_dir}/cmd/check-spelling/kata-spell-check.sh"

	for doc in $docs
	do
		"$cmd" check "$doc" || die "spell check failed for document $doc"
	done
}

# Tests to apply to all files.
#
# Currently just looks for TODO/FIXME comments that should be converted to
# (or annotated with) an Issue URL.
static_check_files()
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

	if [ "$specific_branch" = "true" ]
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

# Perform vendor checks:
#
# - Ensure that changes to vendored code are accompanied by an update to the
#   vendor tooling config file. If not, the user simply hacked the vendor files
#   rather than following the correct process:
#
#   https://github.com/kata-containers/community/blob/master/VENDORING.md
#
# - Ensure vendor metadata is valid.
static_check_vendor()
{
	local files
	local vendor_files
	local result

	# All vendor operations should modify this file
	local vendor_ctl_file="Gopkg.lock"

	[ -e "$vendor_ctl_file" ] || { info "No vendoring in this repository" && return; }

	info "Checking vendored code is pristine"

	files=$(get_pr_changed_file_details_full || true)

	# Strip off status
	files=$(echo "$files"|awk '{print $NF}')

	if [ -n "$files" ]
	then
		# PR changed files so check if it changed any vendored files
		vendor_files=$(echo "$files" | grep "vendor/" || true)

		if [ -n "$vendor_files" ]
		then
			result=$(echo "$files" | egrep "\<${vendor_ctl_file}\>" || true)
			[ -n "$result" ] || die "PR changes vendor files, but does not update ${vendor_ctl_file}"
		fi
	fi

	info "Checking vendoring metadata"

	# Get the vendoring tool
	go get github.com/golang/dep/cmd/dep

	# Check, but don't touch!
	dep ensure -no-vendor -dry-run
}

# Run the specified function (after first checking it is compatible with the
# users architectural preferences), or simply list the function name if list
# mode is active.
run_or_list_check_function()
{
	local name="$1"

	func_is_valid "$name"

	local arch_func
	local handler

	arch_func=$(func_is_arch_specific "$name")

	handler="info"

	# If the user requested only a single function to run, we should die
	# if the function cannot be run due to the other options specified.
	#
	# Whereas if this script is running all functions, just display an
	# info message if a function cannot be run.
	[ "$single_func_only" = "true" ] && handler="die"

	if [ "$handle_funcs" = "arch-agnostic" ] && [ "$arch_func" = "yes" ]; then
		if [ "$list_only" != "true" ]; then
			"$handler" "Not running '$func' as requested no architecture-specific functions"
		fi

		return 0
	fi

	if [ "$handle_funcs" = "arch-specific" ] && [ "$arch_func" = "no" ]; then
		if [ "$list_only" != "true" ]; then
			"$handler" "Not running architecture-agnostic function '$func' as requested only architecture specific functions"
		fi

		return 0
	fi

	if [ "$list_only" = "true" ]; then
		echo "$func"
		return 0
	fi

	info "Running '$func' function"
	eval "$func"
}

main()
{
	trap remove_tmp_files EXIT

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
			--all) specific_branch="true" ;;
			--branch) branch="$2"; shift ;;
			--commits) func=static_check_commits ;;
			--docs) func=static_check_docs ;;
			--files) func=static_check_files ;;
			--force) force="true" ;;
			--golang) func=static_check_go_arch_specific ;;
			-h|--help) usage; exit 0 ;;
			--labels) func=static_check_labels;;
			--licenses) func=static_check_license_headers ;;
			--list) list_only="true" ;;
			--no-arch) handle_funcs="arch-agnostic" ;;
			--only-arch) handle_funcs="arch-specific" ;;
			--repo) repo="$2"; shift ;;
			--vendor) func=static_check_vendor;;
			--versions) func=static_check_versions ;;
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
			if [ "$list_only" != "true" ]; then
				echo >&2 "ERROR: need repo" && usage && exit 1
			fi
		fi
	fi

	local all_check_funcs=$(typeset -F|awk '{print $3}'|grep "${check_func_regex}"|sort)

	# Run user-specified check and quit
	if [ -n "$func" ]; then
		single_func_only="true"
		run_or_list_check_function "$func"
		exit 0
	fi

	for func in $all_check_funcs
	do
		if [ "$func" = "check_commits" ]; then
			if [ -n "$TRAVIS_BRANCH" ] && [ "$TRAVIS_BRANCH" != "master" ]
			then
				echo "Skipping checkcommits"
				echo "See issue: https://github.com/kata-containers/tests/issues/632"
				continue
			fi
		fi

		run_or_list_check_function "$func"
	done
}

main "$@"
