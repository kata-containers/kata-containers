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
typeset -r bash_block_open="\`\`\`bash"
typeset -r block_open="\`\`\`"
typeset -r block_close="\`\`\`"

# GitHub issue templates have a special metadata section at the top delimited
# by this string. See:
#
# https://raw.githubusercontent.com/kata-containers/.github/master/.github/ISSUE_TEMPLATE/bug_report.md
typeset -r metadata_block='---'

# Used to delimit inline code blocks
typeset -r backtick="\`"

# convention used in all documentation to represent a non-privileged users
# shell prompt. All lines starting with this value inside a code block are
# commands the user should run.
typeset -r code_prompt="\$ "

# files are expected to match this regular expression
typeset -r extension_regex="\.md$"

strict="no"
require_commands="no"
check_only="no"
invert="no"
verbose="no"

usage()
{
	cat <<EOF
Usage: ${script_name} [options] <markdown-file> [<script-file> [<description>]]

This script will convert a github-flavoured markdown document file into a
bash(1) script to stdout by extracting the bash code blocks.

Options:

  -c : check the file but don't create the script (sets exit code).
  -h : show this usage.
  -i : invert output (remove code blocks and inline code, displaying the
       remaining parts of the document). Incompatible with '-c'.
  -r : require atleast one command block to be found.
  -s : strict mode - perform extra checks.
  -v : verbose mode.

Example usage:

  $ ${script_name} foo.md foo.md.sh

Notes:

- If a description is specified, it will be added to the script as a
  comment.
- <script-file> may be specified as '-' meaning send output to stdout.

Limitations:

- The script is unable to handle embedded code blocks like this:

  \`\`\`

      \`\`\`bash
      \$ echo code in an embedded set of backticks
      \`\`\`

  \`\`\`

  To overcome this issue, ensure that the outer set of backticks are replaced
  with an HTML PRE tag:

  <pre>

      \`\`\`bash
      \$ echo code in an embedded set of backticks
      \`\`\`

  </pre>

  This will both render correctly on GitHub and allow this script to remove
  the code block.

  Note: this solves one problem but introduces another - this script will not
  remove the HTML tags.

${warning}

EOF

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
	local -r description="$1"

	cat <<-EOF
	#!/bin/bash
	${license}
	#----------------------------------------------
	# WARNING: Script auto-generated from '$file'.
	#
	# ${warning}
	#----------------------------------------------

	#----------------------------------------------
	# Description: $description
	#----------------------------------------------

	# fail the entire script if any simple command fails
	set -e

EOF
}

# Convert the specified github-flavoured markdown format file
# into a bash script by extracting the bash blocks.
doc_to_script()
{
	file="$1"
	outfile="$2"
	description="$3"
	invert="$4"

	[ -n "$file" ] || die "need file"

	[ "${check_only}" = "no" ] && [ -z "$outfile" ] && die "need output file"
	[ "$outfile" = '-' ] && outfile="/dev/stdout"

	if [ "$invert" = "yes" ]
	then
		# First, remove code blocks.
		# Next, remove inline code in backticks.
		# Finally, remove a metadata block as used in GitHub issue
		# templates.
		cat "$file" |\
			sed -e "/^[ \>]*${block_open}/,/^[ \>]*${block_close}/d" \
			    -e "s/${backtick}[^${backtick}]*${backtick}//g" \
			    -e "/^${metadata_block}$/,/^${metadata_block}$/d" \
			     > "$outfile"
		return
	fi

	all=$(mktemp)
	body=$(mktemp)

	cat "$file" |\
		sed -n "/^ *${bash_block_open}/,/^ *${block_close}/ p" |\
		sed -e "/^ *${block_close}/ d" \
		-e "s/^ *${code_prompt}//g" \
		-e 's/^ *//g' > "$body"

	[ "$require_commands" = "yes" ] && [ ! -s "$body" ] && die "no commands found in file '$file'"

	script_header "$description" > "$all"
	cat "$body" >> "$all"

	# sanity check
	[ "$check_only" = "yes" ] && redirect="1>/dev/null 2>/dev/null"

	{ local ret; eval bash -n "$all" $redirect; ret=$?; } || true
	[ "$ret" -ne 0 ] && die "shell code in file '$file' is not valid"

	# create output file
	[ "$check_only" = "no" ] && cp "$all" "$outfile"

	# clean up
	rm -f "$body" "$all"
}

main()
{
	while getopts "chirsv" opt
	do
		case $opt in
			c)	check_only="yes" ;;
			h)	usage ;;
			i)	invert="yes" ;;
			r)	require_commands="yes" ;;
			s)	strict="yes" ;;
			v)	verbose="yes" ;;
		esac
	done

	shift $(($OPTIND - 1))

	file="$1"
	outfile="$2"
	description="$3"

	[ -n "$file" ] || die "need file"

	[ "$verbose" = "yes" ] && echo "INFO: processing file '$file'"

	if [ "$strict" = "yes" ]
	then
		echo "$file"|grep -q "$extension_regex" ||\
			die "file '$file' doesn't match pattern '$extension_regex'"
	fi

	doc_to_script "$file" "$outfile" "$description" "$invert"
}

main "$@"
