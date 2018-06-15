#!/bin/bash

# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0

# Description: Look for tests that are currently being skipped, but which
#   should no longer be (since the issue they are being skipped on is now
#   closed).

set -e

readonly script_name=${0##*/}

info()
{
	local -r msg="$*"

	echo "INFO: $msg"
}

usage()
{
	cat <<EOT
Usage: $script_name [-h | --help | help]

Options:

  --help  : Show this help text.
  -h      :
  help    :

Description: Look for tests in the current repository that contain "skips"
[1]. If the test specifies a skip containing a GitHub issue URL, check its state
since if the issue is (now) closed, the skip is stale and can be removed.

The advantage of removing skips is to increase test coverage and reduce test
code that is not being exercised.

---
[1] - A skip is a test which has been marked as disabled so that it is not
      currently run.
EOT
}

# Determine if the specified github issue is closed.
#
# Warning: This function uses the github API which is rate-limited!
#
# Paramters:
#
#   $1 - full URL (in form https://github.com/$org/$repo/issues/$number)
#
# Returns:
#
# - "yes" if issue is closed.
# - "no" if issue is not closed.
is_github_issue_closed()
{
	local -r url="$1"

	# Convert URL to the API equivalent, which returns a JSON document.
	local -r api_url=$(echo "$url" | sed -e 's|github.com|api.github.com/repos|')

	# Extract issue state from JSON
	local state=$(curl -sL "$api_url" |\
		grep '"state" *:' |\
		cut -d: -f2- |\
		tr -d '"' |\
		tr -d , |\
		tr -d " ")

	[ "$state" = "closed" ] && echo yes && return

	echo "no"
}

check_skips()
{
	[ -n "$1" ] && usage && exit 0

	results=$(mktemp)

	# Get a list of files that contain skips
	grep -ir "skip.*https://github.com/.*/issues/[0-9][0-9]*" > "$results"

	[ ! -e "$results" ] && info "No skipped tests" && rm -f "$results" && return

	count=$(cut -d: -f1 < "$results"|sort -u|wc -l)

	info "Found $count tests containing skips"

	# Extract a unique list of URL from the skip files
	urls=$(xurls "$results"|sort -u)

	url_count=$(echo "$urls"|wc -l)

	info "Found $url_count skip URLs"

	for url in $urls
	do
		info "Checking skip URL $url"

		closed=$(is_github_issue_closed "$url")

		if [ "$closed" = "yes" ]
		then
			# Get a list of files that specify this skip URL
			files=$(grep "$url" "$results"|cut -d: -f1|sort -u)

			info "Remove skip from following files (issue $url is closed):"
			info
			for file in $files
			do
				info "    $file"
			done

			info
		fi
	done

	rm -f "$results"
}

check_skips "$@"
