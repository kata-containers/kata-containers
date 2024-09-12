#!/usr/bin/env bash

# Copyright (c) 2017-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: Central script to run all static checks.
#   This script should be called by all other repositories to ensure
#   there is only a single source of all static checks.

set -e

[ -n "$DEBUG" ] && set -x

cidir=$(realpath $(dirname "$0"))
source "${cidir}/common.bash"

# By default in Golang >= 1.16 GO111MODULE is set to "on",
# some subprojects in this repo may not support "go modules",
# set GO111MODULE to "auto" to enable module-aware mode only when
# a go.mod file is present in the current directory.
export GO111MODULE="auto"
export test_path="${test_path:-github.com/kata-containers/kata-containers/tests}"
export test_dir="${GOPATH}/src/${test_path}"

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
repo_path=""
specific_branch="false"
force="false"
branch=${branch:-main}

# Which static check functions to consider.
handle_funcs="all"

single_func_only="false"
list_only="false"

# number of seconds to wait for curl to check a URL
typeset url_check_timeout_secs="${url_check_timeout_secs:-60}"

# number of attempts that will be made to check an individual URL.
typeset url_check_max_tries="${url_check_max_tries:-3}"

typeset -A long_options

# Generated code
ignore_clh_generated_code="virtcontainers/pkg/cloud-hypervisor/client"

paths_to_skip=(
	"${ignore_clh_generated_code}"
	"vendor"
)

# Skip paths that are not statically checked
# $1 : List of paths to check, space separated list
# If you have a list in a bash array call in this way:
# list=$(skip_paths "${list[@]}")
# If you still want to use it as an array do:
# list=(${list})
skip_paths(){
	local list_param="${1}"
	[ -z "$list_param" ] && return
	local list=(${list_param})

	for p in "${paths_to_skip[@]}"; do
		new_list=()
		for l in "${list[@]}"; do
			if echo "${l}" | grep -qv "${p}"; then
				new_list=("${new_list[@]}" "${l}")
			fi
		done
		list=("${new_list[@]}")
	done
	echo "${list[@]}"
}


long_options=(
	[all]="Force checking of all changes, including files in the base branch"
	[branch]="Specify upstream branch to compare against (default '$branch')"
	[docs]="Check document files"
	[dockerfiles]="Check dockerfiles"
	[files]="Check files"
	[force]="Force a skipped test to run"
	[golang]="Check '.go' files"
	[help]="Display usage statement"
	[json]="Check JSON files"
	[labels]="Check labels databases"
	[licenses]="Check licenses"
	[list]="List tests that would run"
	[no-arch]="Run/list all tests except architecture-specific ones"
	[only-arch]="Only run/list architecture-specific tests"
	[repo:]="Specify GitHub URL of repo to use (github.com/user/repo)"
	[scripts]="Check script files"
	[vendor]="Check vendor files"
	[versions]="Check versions files"
	[xml]="Check XML files"
)

yamllint_cmd="yamllint"
have_yamllint_cmd=$(command -v "$yamllint_cmd" || true)

chronic=chronic

# Disable chronic on OSX to avoid having to update the Travis config files
# for additional packages on that platform.
[ "$(uname -s)" == "Darwin" ] && chronic=

usage()
{
	cat <<EOF

Usage: $script_name help
       $script_name [options] repo-name [true]

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

	cat <<EOF

Parameters:

  help      : Show usage.
  repo-name : GitHub URL of repo to check in form "github.com/user/repo"
              (equivalent to "--repo \$URL").
  true      : Specify as "true" if testing a specific branch, else assume a
              PR branch (equivalent to "--all").

Notes:

- If no options are specified, all non-skipped tests will be run.
- Some required tools may be installed in \$GOPATH/bin, so you should ensure
  that it is in your \$PATH.

Examples:

- Run all tests on a specific branch (stable or main) of kata-containers repo:

  $ $script_name github.com/kata-containers/kata-containers true

- Auto-detect repository and run golang tests for current repository:

  $ KATA_DEV_MODE=true $script_name --golang

- Run all tests on the kata-containers repository, forcing the tests to
  consider all files, not just those changed by a PR branch:

  $ $script_name github.com/kata-containers/kata-containers --all


EOF
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

# Check that chronic is installed, otherwise die.
need_chronic() {
	local first_word
	[ -z "$chronic" ] && return
	first_word="${chronic%% *}"
	command -v chronic &>/dev/null || \
		die "chronic command not found. You must have it installed to run this check." \
		"Usually it is distributed with the 'moreutils' package of your Linux distribution."
}


static_check_go_arch_specific()
{
	local go_packages
	local submodule_packages
	local all_packages

	pushd $repo_path

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
	go_packages=$(comm -3 "$all_packages" "$submodule_packages" || true)
	go_packages=$(skip_paths "${go_packages[@]}")

	# No packages to test
	[ -z "$go_packages" ] && popd && return

	local linter="golangci-lint"

	# Run golang checks
	if [ ! "$(command -v $linter)" ]
	then
		info "Installing ${linter}"

		local linter_url=$(get_test_version "languages.golangci-lint.url")
		local linter_version=$(get_test_version "languages.golangci-lint.version")

		info "Forcing ${linter} version ${linter_version}"
		curl -sSfL https://raw.githubusercontent.com/golangci/golangci-lint/master/install.sh | sh -s -- -b $(go env GOPATH)/bin "v${linter_version}"
		command -v $linter &>/dev/null || \
			die "$linter command not found. Ensure that \"\$GOPATH/bin\" is in your \$PATH."
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
	for d in ${dirs};do
		info "Running $linter on $d"
		(cd $d && GO111MODULE=auto eval "$linter" "${linter_args}" ".")
	done
	popd

}

# Install yamllint in the different Linux distributions
install_yamllint()
{
	package="yamllint"

	case "$ID" in
		centos|rhel) sudo yum -y install $package ;;
		ubuntu) sudo apt-get -y install $package ;;
		fedora) sudo dnf -y install $package ;;
		*) die "Please install yamllint on $ID" ;;
	esac

	have_yamllint_cmd=$(command -v "$yamllint_cmd" || true)

	if [ -z "$have_yamllint_cmd" ]; then
		info "Cannot install $package" && return
	fi
}

# Check the "versions database".
#
# Some repositories use a versions database to maintain version information
# about non-golang dependencies. If found, check it for validity.
static_check_versions()
{
	local db="versions.yaml"

	if [ -z "$have_yamllint_cmd" ]; then
		info "Installing yamllint"
		install_yamllint
	fi

	pushd $repo_path

	[ ! -e "$db" ] && popd && return

	if [ -n "$have_yamllint_cmd" ]; then
		eval "$yamllint_cmd" "$db"
	else
		info "Cannot check versions as $yamllint_cmd not available"
	fi

	popd
}

static_check_labels()
{
	[ $(uname -s) != Linux ] && info "Can only check labels under Linux" && return

	# Handle SLES which doesn't provide the required command.
	[ -z "$have_yamllint_cmd" ] && info "Cannot check labels as $yamllint_cmd not available" && return

	# Since this script is called from another repositories directory,
	# ensure the utility is built before the script below (which uses it) is run.
	(cd "${test_dir}/cmd/github-labels" && make)

	tmp=$(mktemp)

	files_to_remove+=("${tmp}")

	info "Checking labels for repo ${repo} using temporary combined database ${tmp}"

	bash -f "${test_dir}/cmd/github-labels/github-labels.sh" "generate" "${repo}" "${tmp}"
}

# Ensure all files (where possible) contain an SPDX license header
static_check_license_headers()
{
	# The branch is the baseline - ignore it.
	[ "$specific_branch" = "true" ] && return

	# See: https://spdx.org/licenses/Apache-2.0.html
	local -r spdx_tag="SPDX-License-Identifier"
	local -r spdx_license="Apache-2.0"
	local -r license_pattern="${spdx_tag}: ${spdx_license}"
	local -r copyright_pattern="Copyright"

	local header_checks=()

	header_checks+=("SPDX license header::${license_pattern}")
	header_checks+=("Copyright header:-i:${copyright_pattern}")

	pushd $repo_path

	files=$(get_pr_changed_file_details || true)

	# Strip off status
	files=$(echo "$files"|awk '{print $NF}')

	# no files were changed
	[ -z "$files" ] && info "No files found" && popd && return

	local header_check

	for header_check in "${header_checks[@]}"
	do
		local desc=$(echo "$header_check"|cut -d: -f1)
		local extra_args=$(echo "$header_check"|cut -d: -f2)
		local pattern=$(echo "$header_check"|cut -d: -f3-)

		info "Checking $desc"

		local missing=$(grep \
			--exclude=".git/*" \
			--exclude=".gitignore" \
			--exclude=".dockerignore" \
			--exclude="Gopkg.lock" \
			--exclude="*.gpl.c" \
			--exclude="*.ipynb" \
			--exclude="*.jpg" \
			--exclude="*.json" \
			--exclude="LICENSE*" \
			--exclude="THIRD-PARTY" \
			--exclude="*.md" \
			--exclude="*.pb.go" \
			--exclude="*pb_test.go" \
			--exclude="*.bin" \
			--exclude="*.png" \
			--exclude="*.pub" \
			--exclude="*.service" \
			--exclude="*.svg" \
			--exclude="*.drawio" \
			--exclude="*.toml" \
			--exclude="*.txt" \
			--exclude="*.dtd" \
			--exclude="vendor/*" \
			--exclude="VERSION" \
			--exclude="kata_config_version" \
			--exclude="tools/packaging/kernel/configs/*" \
			--exclude="virtcontainers/pkg/firecracker/*" \
			--exclude="${ignore_clh_generated_code}*" \
			--exclude="*.xml" \
			--exclude="*.yaml" \
			--exclude="*.yml" \
			--exclude="go.mod" \
			--exclude="go.sum" \
			--exclude="*.lock" \
			--exclude="grpc-rs/*" \
			--exclude="target/*" \
			--exclude="*.patch" \
			--exclude="*.diff" \
			--exclude="tools/packaging/static-build/qemu.blacklist" \
			--exclude="tools/packaging/qemu/default-configs/*" \
			--exclude="src/libs/protocols/protos/gogo/*.proto" \
			--exclude="src/libs/protocols/protos/google/*.proto" \
			--exclude="src/libs/protocols/protos/cri-api/api.proto" \
			--exclude="src/mem-agent/example/protocols/protos/google/protobuf/*.proto" \
			--exclude="src/libs/*/test/texture/*" \
			--exclude="*.dic" \
			-EL $extra_args -E "\<${pattern}\>" \
			$files || true)

		if [ -n "$missing" ]; then
			cat >&2 <<-EOF
		ERROR: Required $desc check ('$pattern') failed for the following files:

		$missing

EOF
			exit 1
		fi
	done
	popd
}

run_url_check_cmd()
{
	local url="${1:-}"
	[ -n "$url" ] || die "need URL"

	local out_file="${2:-}"
	[ -n "$out_file" ] || die "need output file"

	# Can be blank
	local extra_args="${3:-}"

	local curl_extra_args=()

	curl_extra_args+=("$extra_args")

	# Authenticate for github to increase threshold for rate limiting
	if [[ "$url" =~ github\.com && -n "$GITHUB_USER" && -n "$GITHUB_TOKEN" ]]; then
		curl_extra_args+=("-u ${GITHUB_USER}:${GITHUB_TOKEN}")
	fi

	# Some endpoints return 403 to HEAD but 200 for GET,
	# so perform a GET but only read headers.
	curl \
		${curl_extra_args[*]} \
		-sIL \
		-X GET \
		-c - \
		-H "Accept-Encoding: zstd, none, gzip, deflate" \
		--max-time "$url_check_timeout_secs" \
		--retry "$url_check_max_tries" \
		"$url" \
		&>"$out_file"
}

check_url()
{
	local url="${1:-}"
	[ -n "$url" ] || die "need URL to check"

	local invalid_urls_dir="${2:-}"
	[ -n "$invalid_urls_dir" ] || die "need invalid URLs directory"

	local curl_out
	curl_out=$(mktemp)

	files_to_remove+=("${curl_out}")

	# Process specific file to avoid out-of-order writes
	local invalid_file
	invalid_file=$(printf "%s/%d" "$invalid_urls_dir" "$$")

	local ret

	local -a errors=()

	local -a user_agents=()

	# Test an unspecified UA (curl default)
	user_agents+=('')

	# Test an explictly blank UA
	user_agents+=('""')

	# Single space
	user_agents+=(' ')

	# CLI HTTP tools
	user_agents+=('Wget')
	user_agents+=('curl')

	# console based browsers
	# Hopefully, these will always be supported for a11y.
	user_agents+=('Lynx')
	user_agents+=('Elinks')

	# Emacs' w3m browser
	user_agents+=('Emacs')

	# The full craziness
	user_agents+=('Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/103.0.0.0 Safari/537.36')

	local user_agent

	# Cycle through the user agents until we find one that works.
	#
	# Note that we also test an unspecified user agent
	# (no '-A <value>').
	for user_agent in "${user_agents[@]}"
	do
		info "Checking URL $url with User Agent '$user_agent'"

		local curl_ua_args
		[ -n "$user_agent" ] && curl_ua_args="-A '$user_agent'"

		{ run_url_check_cmd "$url" "$curl_out" "$curl_ua_args"; ret=$?; } || true

		# A transitory error, or the URL is incorrect,
		# but capture either way.
		if [ "$ret" -ne 0 ]; then
			errors+=("Failed to check URL '$url' (user agent: '$user_agent', return code $ret)")

			# Try again with another UA since it appears that some return codes
			# indicate the server was unhappy with the details
			# presented by the client.
			continue
		fi

		local http_statuses

		http_statuses=$(grep -E "^HTTP" "$curl_out" |\
			awk '{print $2}' || true)

		if [ -z "$http_statuses" ]; then
			errors+=("no HTTP status codes for URL '$url' (user agent: '$user_agent')")

			continue
		fi

		local status

		local -i fail_count=0

		# Check all HTTP status codes
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

			{ grep -qE "^(1[0-9][0-9]|2[0-9][0-9]|3[0-9][0-9]|405)" <<< "$status"; ret=$?; } || true

			[ "$ret" -eq 0 ] && continue

			fail_count+=1
		done

		# If we didn't receive any unexpected HTTP status codes for
		# this UA, the URL is valid so we don't need to check with any
		# further UAs, so clear any (transitory) errors we've
		# recorded.
		[ "$fail_count" -eq 0 ] && errors=() && break

		echo "$url" >> "$invalid_file"
		errors+=("found HTTP error status codes for URL $url (status: '$status', user agent: '$user_agent')")
	done

	[ "${#errors}" = 0 ] && return 0

	die "failed to check URL '$url': errors: '${errors[*]}'"
}

# Perform basic checks on documentation files
static_check_docs()
{
	local cmd="xurls"

	pushd $repo_path

	if [ ! "$(command -v $cmd)" ]
	then
		info "Installing $cmd utility"

		local version
		local url

		version=$(get_test_version "externals.xurls.version")
		url=$(get_test_version "externals.xurls.url")

		# xurls is very fussy about how it's built.
		go install "${url}@${version}"

		command -v xurls &>/dev/null ||
			die 'xurls not found. Ensure that "$GOPATH/bin" is in your $PATH'
	fi

	info "Checking documentation"

	local doc
	local all_docs
	local docs
	local docs_status
	local new_docs
	local new_urls
	local url

	pushd $repo_path

	all_docs=$(git ls-files "*.md" | grep -Ev "(grpc-rs|target)/" | sort || true)
	all_docs=$(skip_paths "${all_docs[@]}")

	if [ "$specific_branch" = "true" ]
	then
		info "Checking all documents in $branch branch"
		docs="$all_docs"
	else
		info "Checking local branch for changed documents only"

		docs_status=$(get_pr_changed_file_details || true)
		docs_status=$(echo "$docs_status" | grep "\.md$" || true)

		docs=$(echo "$docs_status" | awk '{print $NF}' | sort)
		docs=$(skip_paths "${docs[@]}")

		# Newly-added docs
		new_docs=$(echo "$docs_status" | awk '/^A/ {print $NF}' | sort)
		new_docs=$(skip_paths "${new_docs[@]}")

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

	command -v kata-check-markdown &>/dev/null ||\
		(cd "${test_dir}/cmd/check-markdown" && make)

	command -v kata-check-markdown &>/dev/null || \
		die 'kata-check-markdown command not found. Ensure that "$GOPATH/bin" is in your $PATH.'

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

	# Exclude READMEs for test integration
	exclude_doc_regexs+=(^\tests/cmd/.*/README\.md$)

	local exclude_pattern

	# Convert the list of files into an grep(1) alternation pattern.
	exclude_pattern=$(echo "${exclude_doc_regexs[@]}"|sed 's, ,|,g')

	# Every document in the repo (except a small handful of exceptions)
	# should be referenced by another document.
	for doc in $md_docs_to_check
	do
		# Check the ignore list for markdown files that do not need to
		# be referenced by others.
		echo "$doc"|grep -q -E "(${exclude_pattern})" && continue

		grep -q "$doc" "$md_links" || die "Document $doc is not referenced"
	done

	info "Checking document code blocks"

	local doc_to_script_cmd="${cidir}/kata-doc-to-script.sh"

	for doc in $docs
	do
		bash "${doc_to_script_cmd}" -csv "$doc"

		# Look for URLs in the document
		urls=$("${doc_to_script_cmd}" -i "$doc" - | "$cmd")

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
			echo "$new_urls" | grep -q -E "\<${url}\>" && \
				info "ignoring new (but correct) URL: $url" && continue
		fi

		# Ignore local URLs. The only time these are used is in
		# examples (meaning these URLs won't exist).
		echo "$url" | grep -q "^file://" && continue
		echo "$url" | grep -q "^http://localhost" && continue

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
	cmd="${test_dir}/cmd/check-spelling/kata-spell-check.sh"

	local docs_failed=0
	for doc in $docs
	do
		"$cmd" check "$doc" || { info "spell check failed for document $doc" && docs_failed=1; }

		static_check_eof "$doc"
	done

	popd

	[ $docs_failed -eq 0 ] || {
        url='https://github.com/kata-containers/kata-containers/blob/main/docs/Documentation-Requirements.md#spelling'
        die "spell check failed, See $url for more information."
    }
}

static_check_eof()
{
	local file="$1"
	local anchor="EOF"


	[ -z "$file" ] && info "No files to check" && return

	# Skip the itself
	[ "$file" == "$script_name" ] && return

	# Skip the Vagrantfile
	[ "$file" == "Vagrantfile" ] && return

	local invalid=$(cat "$file" |\
		grep -o -E '<<-* *\w*' |\
		sed -e 's/^<<-*//g' |\
		tr -d ' ' |\
		sort -u |\
		grep -v -E '^$' |\
		grep -v -E "$anchor" || true)
	[ -z "$invalid" ] || die "Expected '$anchor' here anchor, in $file found: $invalid"
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

		files=$(git ls-files | grep -v -E "/(.git|vendor|grpc-rs|target)/" || true)
	else
		info "Checking local branch for changed files only"

		files=$(get_pr_changed_file_details || true)

		# Strip off status
		files=$(echo "$files"|awk '{print $NF}')
	fi

	[ -z "$files" ] && info "No files changed" && return

	local matches=""

	pushd $repo_path

	for file in $files
	do
		local match

		# Look for files containing the specified comment tags but
		# which do not include a github URL.
		match=$(grep -H -E "\<FIXME\>|\<TODO\>" "$file" |\
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

	popd

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
#   https://github.com/kata-containers/community/blob/main/VENDORING.md
#
# - Ensure vendor metadata is valid.
static_check_vendor()
{
	pushd $repo_path

	local files
	local files_arr=()

	files=$(find . -type f -name "go.mod")

	while IFS= read -r line; do
		files_arr+=("$line")
	done <<< "$files"

	for file in "${files_arr[@]}"; do
	        local dir=$(echo $file | sed 's/go\.mod//')

	        pushd $dir

		# Check if directory has been changed to use go modules
		if [ -f "go.mod" ]; then
			info "go.mod file found in $dir, running go mod verify instead"
			# This verifies the integrity of modules in the local cache.
			# This does not really verify the integrity of vendored code:
			# https://github.com/golang/go/issues/27348
			# Once that is added we need to add an extra step to verify vendored code.
			go mod verify
		fi
		popd
	done

	popd
}

static_check_xml()
{
	local all_xml
	local files

	pushd $repo_path

	need_chronic

	all_xml=$(git ls-files "*.xml" | grep -Ev "/(vendor|grpc-rs|target)/" | sort || true)

	if [ "$specific_branch" = "true" ]
	then
		info "Checking all XML files in $branch branch"
		files="$all_xml"
	else
		info "Checking local branch for changed XML files only"

		local xml_status

		xml_status=$(get_pr_changed_file_details || true)
		xml_status=$(echo "$xml_status" | grep "\.xml$" || true)

		files=$(echo "$xml_status" | awk '{print $NF}')
	fi

	[ -z "$files" ] && info "No XML files to check" && popd && return

	local file

	for file in $files
	do
		info "Checking XML file '$file'"

		local contents

		# Most XML documents are specified as XML 1.0 since, with the
		# advent of XML 1.0 (Fifth Edition), XML 1.1 is "almost
		# redundant" due to XML 1.0 providing the majority of XML 1.1
		# features. xmllint doesn't support XML 1.1 seemingly for this
		# reason, so the only check we can do is to (crudely) force
		# the document to be an XML 1.0 one since XML 1.1 documents
		# can mostly be represented as XML 1.0.
		#
		# This is only really required since Jenkins creates XML 1.1
		# documents.
		contents=$(sed "s/xml version='1.1'/xml version='1.0'/g" "$file")

		local ret

		{ $chronic xmllint -format - <<< "$contents"; ret=$?; } || true

		[ "$ret" -eq 0 ] || die "failed to check XML file '$file'"
	done

	popd
}

static_check_shell()
{
	local all_scripts
	local scripts

	pushd $repo_path

	need_chronic

	all_scripts=$(git ls-files "*.sh" "*.bash" | grep -Ev "/(vendor|grpc-rs|target)/" | sort || true)

	if [ "$specific_branch" = "true" ]
	then
		info "Checking all scripts in $branch branch"
		scripts="$all_scripts"
	else
		info "Checking local branch for changed scripts only"

		local scripts_status
		scripts_status=$(get_pr_changed_file_details || true)
		scripts_status=$(echo "$scripts_status" | grep -E "\.(sh|bash)$" || true)

		scripts=$(echo "$scripts_status" | awk '{print $NF}')
	fi

	[ -z "$scripts" ] && info "No scripts to check" && popd && return 0

	local script

	for script in $scripts
	do
		info "Checking script file '$script'"

		local ret

		{ $chronic bash -n "$script"; ret=$?; } || true

		[ "$ret" -eq 0 ] || die "check for script '$script' failed"

		static_check_eof "$script"
	done

	popd
}

static_check_json()
{
	local all_json
	local json_files

	pushd $repo_path

	need_chronic

	all_json=$(git ls-files "*.json" | grep -Ev "/(vendor|grpc-rs|target)/" | sort || true)

	if [ "$specific_branch" = "true" ]
	then
		info "Checking all JSON in $branch branch"
		json_files="$all_json"
	else
		info "Checking local branch for changed JSON only"

		local json_status
		json_status=$(get_pr_changed_file_details || true)
		json_status=$(echo "$json_status" | grep "\.json$" || true)

		json_files=$(echo "$json_status" | awk '{print $NF}')
	fi

	[ -z "$json_files" ] && info "No JSON files to check" && popd && return 0

	local json

	for json in $json_files
	do
		info "Checking JSON file '$json'"

		local ret

		{ $chronic jq -S . "$json"; ret=$?; } || true

		[ "$ret" -eq 0 ] || die "failed to check JSON file '$json'"
	done

	popd
}

# The dockerfile checker relies on the hadolint tool. This function handle its
# installation if it is not found on PATH.
# Note that we need a specific version of the tool as it seems to not have
# backward/forward compatibility between versions.
has_hadolint_or_install()
{
	# Global variable set by the caller. It might be overwritten here.
	linter_cmd=${linter_cmd:-"hadolint"}
	local linter_version=$(get_test_version "externals.hadolint.version")
	local linter_url=$(get_test_version "externals.hadolint.url")
	local linter_dest="${GOPATH}/bin/hadolint"

	local has_linter=$(command -v "$linter_cmd")
	if [[ -z "$has_linter" && "$KATA_DEV_MODE" == "yes" ]]; then
		# Do not install if it is in development mode.
		die "$linter_cmd command not found. You must have the version $linter_version installed to run this check."
	elif [ -n "$has_linter" ]; then
		# Check if the expected linter version
		if $linter_cmd --version | grep -v "$linter_version" &>/dev/null; then
			warn "$linter_cmd command found but not the required version $linter_version"
			has_linter=""
		fi
	fi

	if [ -z "$has_linter" ]; then
		local download_url="${linter_url}/releases/download/v${linter_version}/hadolint-Linux-x86_64"
		info "Installing $linter_cmd $linter_version at $linter_dest"

		curl -sfL "$download_url" -o "$linter_dest" || \
			die "Failed to download $download_url"
		chmod +x "$linter_dest"

		# Overwrite in case it cannot be found in PATH.
		linter_cmd="$linter_dest"
	fi
}

static_check_dockerfiles()
{
	local all_files
	local files
	local ignore_files
	# Put here a list of files which should be ignored.
        local ignore_files=(
        )

	pushd $repo_path

	local linter_cmd="hadolint"

        all_files=$(git ls-files "*/Dockerfile*" | grep -Ev "/(vendor|grpc-rs|target)/" | sort || true)

        if [ "$specific_branch" = "true" ]; then
                info "Checking all Dockerfiles in $branch branch"
		files="$all_files"
        else
                info "Checking local branch for changed Dockerfiles only"

                local files_status
		files_status=$(get_pr_changed_file_details || true)
		files_status=$(echo "$files_status" | grep -E "Dockerfile.*$" || true)

		files=$(echo "$files_status" | awk '{print $NF}')
        fi

        [ -z "$files" ] && info "No Dockerfiles to check" && popd && return 0

	# As of this writing hadolint is only distributed for x86_64
	if [ "$(uname -m)" != "x86_64" ]; then
		info "Skip checking as $linter_cmd is not available for $(uname -m)"
		popd
		return 0
	fi
	has_hadolint_or_install

	linter_cmd+=" --no-color"

	# Let's not fail with INFO rules.
	linter_cmd+=" --failure-threshold warning"

	# Some rules we don't want checked, below we ignore them.
	#
	# "DL3008 warning: Pin versions in apt get install"
	linter_cmd+=" --ignore DL3008"
	# "DL3041 warning: Specify version with `dnf install -y <package>-<version>`"
	linter_cmd+=" --ignore DL3041"
	# "DL3033 warning: Specify version with `yum install -y <package>-<version>`"
	linter_cmd+=" --ignore DL3033"
	# "DL3018 warning: Pin versions in apk add. Instead of `apk add <package>` use `apk add <package>=<version>`"
	linter_cmd+=" --ignore DL3018"
	# "DL3003 warning: Use WORKDIR to switch to a directory"
	# See https://github.com/hadolint/hadolint/issues/70
	linter_cmd+=" --ignore DL3003"
	# "DL3048 style: Invalid label key"
	linter_cmd+=" --ignore DL3048"
        # DL3037 warning: Specify version with `zypper install -y <package>=<version>`.
	linter_cmd+=" --ignore DL3037"

	# Temporary add to prevent failure for test migration
	# DL3040 warning: `dnf clean all` missing after dnf command.
	linter_cmd+=" --ignore DL3040"

	local file
	for file in $files; do
		if echo "${ignore_files[@]}" | grep -q $file ; then
			info "Ignoring Dockerfile '$file'"
			continue
		fi

		info "Checking Dockerfile '$file'"
		local ret
		# The linter generates an Abstract Syntax Tree (AST) from the
		# dockerfile. Some of our dockerfiles are actually templates
		# with special syntax, thus the linter might fail to build
		# the AST. Here we handle Dockerfile templates.
		if [[ "$file" =~ Dockerfile.*\.(in|template)$ ]]; then
			# In our templates, text with marker as @SOME_NAME@ is
			# replaceable. Usually it is used to replace in a
			# FROM command (e.g. `FROM @UBUNTU_REGISTRY@/ubuntu`)
			# but also to add an entire block of commands. Example
			# of later:
			# ```
			# RUN apt-get install -y package1
			# @INSTALL_MUSL@
			# @INSTALL_RUST@
			# ```
			# It's known that the linter will fail to parse lines
			# started with `@`. Also it might give false-positives
		        # on some cases. Here we remove all markers as a best
			# effort approach. If the template file is still
			# unparseable then it should be added in the
			# `$ignore_files` list.
			{ sed -e 's/^@[A-Z_]*@//' -e 's/@\([a-zA-Z_]*\)@/\1/g' "$file" | $linter_cmd -; ret=$?; }\
				|| true
		else
			# Non-template Dockerfile.
			{ $linter_cmd "$file"; ret=$?; } || true
		fi

		[ "$ret" -eq 0 ] || die "failed to check Dockerfile '$file'"
	done
	popd
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

setup()
{
	source /etc/os-release || source /usr/lib/os-release

	trap remove_tmp_files EXIT
}

# Display a message showing some system details.
announce()
{
	local arch
	arch=$(uname -m)

	local file='/proc/cpuinfo'

	local detail
	detail=$(grep -m 1 -E '\<vendor_id\>|\<cpu\> *	*:' "$file" \
		2>/dev/null |\
		cut -d: -f2- |\
		tr -d ' ' || true)

	local arch="$arch"

	[ -n "$detail" ] && arch+=" ('$detail')"

	local kernel
	kernel=$(uname -r)

	local distro_name
	local distro_version

	distro_name="${NAME:-}"
	distro_version="${VERSION:-}"

	local -a lines

	local IFS=$'\n'

    lines=( $(cat <<-EOF
	Running static checks:
	  script: $script_name
	  architecture: $arch
	  kernel: $kernel
	  distro:
	    name: $distro_name
	    version: $distro_version
	EOF
	))

	local line

	for line in "${lines[@]}"
	do
		info "$line"
	done
}

main()
{
	setup

	local long_option_names="${!long_options[@]}"

	local args

	args=$(getopt \
		-n "$script_name" \
		-a \
		--options="h" \
		--longoptions="$long_option_names" \
		-- "$@")
	[ $? -eq 0 ] || { usage >&2; exit 1; }

	eval set -- "$args"

	local func=

	while [ $# -gt 1 ]
	do
		case "$1" in
			--all) specific_branch="true" ;;
			--branch) branch="$2"; shift ;;
			--commits) func=static_check_commits ;;
			--docs) func=static_check_docs ;;
			--dockerfiles) func=static_check_dockerfiles ;;
			--files) func=static_check_files ;;
			--force) force="true" ;;
			--golang) func=static_check_go_arch_specific ;;
			-h|--help) usage; exit 0 ;;
			--json) func=static_check_json ;;
			--labels) func=static_check_labels;;
			--licenses) func=static_check_license_headers ;;
			--list) list_only="true" ;;
			--no-arch) handle_funcs="arch-agnostic" ;;
			--only-arch) handle_funcs="arch-specific" ;;
			--repo) repo="$2"; shift ;;
			--scripts) func=static_check_shell ;;
			--vendor) func=static_check_vendor;;
			--versions) func=static_check_versions ;;
			--xml) func=static_check_xml ;;
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

	repo_path=$GOPATH/src/$repo

	announce

	local all_check_funcs=$(typeset -F|awk '{print $3}'|grep "${check_func_regex}"|sort)

	# Run user-specified check and quit
	if [ -n "$func" ]; then
		single_func_only="true"
		run_or_list_check_function "$func"
		exit 0
	fi

	for func in $all_check_funcs
	do
		run_or_list_check_function "$func"
	done
}

main "$@"
