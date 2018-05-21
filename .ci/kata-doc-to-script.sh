#!/bin/bash
license="
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
"

set -e

[ -n "$DEBUG" ] && set -x

script_name="${0##*/}"

typeset -r warning="WARNING: Do *NOT* run the generated script without reviewing it carefully first!"

# github markdown markers used to surround a code block. All text within the
# markers is rendered in a fixed font.
typeset -r block_open="\`\`\`bash"
typeset -r block_close="\`\`\`"

# convention used in all documentation to represent a non-privileged users
# shell prompt. All lines starting with this value inside a code block are
# commands the user should run.
typeset -r code_prompt="\$ "

# files are expected to match this regular expression
typeset -r extension_regex="\.md$"

strict="no"
require_commands="no"
check_only="no"
verbose="no"

usage()
{
	cat <<EOT
Usage: ${script_name} [options] <markdown-file> [<script-file>]

This script will convert a github-flavoured markdown document file into a
bash(1) script to stdout by extracting the bash code blocks.


Options:

  -c : check the file but don't create the script (sets exit code).
  -h : show this usage.
  -r : require atleast one command block to be found.
  -s : strict mode - perform extra checks.
  -v : verbose mode.

${warning}

Example usage:

  $ ${script_name} foo.md foo.md.sh

${warning}

EOT

	exit 0
}

die()
{
	local msg="$*"

	echo "ERROR: $msg" >&2
	exit 1
}

script_header()
{
	cat <<-EOT
	#!/bin/bash
	${license}
	#----------------------------------------------
	# WARNING: Script auto-generated from '$file'.
	#
	# ${warning}
	#----------------------------------------------

	# fail the entire script if any simple command fails
	set -e

EOT
}

# Convert the specified github-flavoured markdown format file
# into a bash script by extracting the bash blocks.
doc_to_script()
{
	file="$1"
	outfile="$2"

	[ -n "$file" ] || die "need file"

	[ "${check_only}" = "no" ] && [ -z "$outfile" ] && die "need output file"

	all=$(mktemp)
	body=$(mktemp)

	cat "$file" |\
		sed -n "/^${block_open}/,/^${block_close}/ p" |\
		sed -e "/^${block_close}/ d" \
		-e "s/^${code_prompt}//g" > "$body"

	[ "$require_commands" = "yes" ] && [ ! -s "$body" ] && die "no commands found in file '$file'"

	script_header > "$all"
	cat "$body" >> "$all"

	# sanity check
	[ "$check_only" = "yes" ] && redirect="1>/dev/null 2>/dev/null"

	eval bash -n "$all" $redirect

	# create output file
	[ "$check_only" = "no" ] && cp "$all" "$outfile"

	# clean up
	rm -f "$body" "$all"
}

main()
{
	while getopts "chrsv" opt
	do
		case $opt in
			c)	check_only="yes" ;;
			h)	usage ;;
			r)	require_commands="yes" ;;
			s)	strict="yes" ;;
			v)	verbose="yes" ;;
		esac
	done

	shift $(($OPTIND - 1))

	file="$1"
	outfile="$2"

	[ -n "$file" ] || die "need file"

	[ "$verbose" = "yes" ] && echo "INFO: processing file '$file'"

	if [ "$strict" = "yes" ]
	then
		echo "$file"|grep -q "$extension_regex" ||\
			die "file '$file' doesn't match pattern '$extension_regex'"
	fi

	doc_to_script "$file" "$outfile"
}

main "$@"
