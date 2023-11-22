#!/bin/bash
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
# Description: spell-check utility.

[ -n "$DEBUG" ] && set -x

set -o errexit
set -o pipefail
set -o nounset

# Ensure we spell check in English
LANG=C
LC_ALL=C

script_name=${0##*/}

if [ "$(uname -s)" == "Darwin" ]
then
	# Hunspell dictionaries are a not easily available
	# on this platform it seems.
	echo "INFO: $script_name: OSX not supported - exiting"
	exit 0
fi

self_dir=$(dirname "$(readlink -f "$0")")
cidir="${self_dir}/../../.ci"

source "${cidir}/lib.sh"

# Directory containing word lists.
#
# Each file in this directory must:
#
# - Have the ".txt" extension.
# - Contain one word per line.
#
# Additionally, the files may contain blank lines and comments
# (lines beginning with '#').
KATA_DICT_FRAGMENT_DIR=${KATA_DICT_FRAGMENT_DIR:-data}

KATA_DICT_NAME="${KATA_DICT_NAME:-kata-dictionary}"

# Name of dictionary file suitable for using with hunspell(1)
# as a personal dictionary.
KATA_DICT_FILE="${KATA_DICT_FILE:-${KATA_DICT_NAME}.dic}"

KATA_RULES_FILE="${KATA_RULES_FILE:-${KATA_DICT_FILE/.dic/.aff}}"

# command to remove code from markdown (inline and blocks)
strip_cmd="${cidir}/kata-doc-to-script.sh"

fragment_dir="${self_dir}/${KATA_DICT_FRAGMENT_DIR}"

# Name of file containing dictionary rules that apply to the
# KATA_DICT_FILE word list.
rules_file_name="rules.aff"

# Command to spell check a file
spell_check_cmd="${KATA_SPELL_CHECK_CMD:-hunspell}"

# Command to convert a markdown file into plain text
md_convert_tool="${KATA_MARKDOWN_CONVERT_TOOL:-pandoc}"

KATA_DICT_DIR="${KATA_DICT_DIR:-${self_dir}}"
dict_file="${KATA_DICT_DIR}/${KATA_DICT_FILE}"
rules_file="${KATA_DICT_DIR}/${KATA_RULES_FILE}"

# Hunspell refers to custom dictionary by their path followed by the name of
# the dictionary (without the file extension).
kata_dict_ref="${KATA_DICT_DIR}/${KATA_DICT_NAME}"

# All project documentation must be written in English,
# with American English taking priority.
#
# We also use a custom dictionary which has to be specified by its
# "directory and name prefix" and which must also be the first specified
# dictionary.
dict_languages="${kata_dict_ref},en_US,en_GB"

make_dictionary()
{
	[ -d "$fragment_dir" ] || die "invalid fragment directory"
	[ -z "$dict_file" ] && die "missing dictionary output file name"

	# Note: the first field is extracted to allow for inline
	# comments in each fragment. For example:
	#
	#  word # this text describes why the word is in the dictionary.
	#
	local dict

	dict=$(cat "$fragment_dir"/*.txt |\
		grep -v '^\#' |\
		grep -v '^$' |\
		awk '{print $1}' |\
		sort -u || true)

	[ -z "$dict" ] && die "generated dictionary is empty"

	# Now, add in the number of words as a header (required by Hunspell)
	local count

	count=$(echo "$dict"| wc -l | awk '{print $1}' || true)
	[ -z "$count" ] && die "cannot determine dictionary length"
	[ "$count" -eq 0 ] && die "invalid dictionary length"

	# Construct the dictionary
	(echo "$count"; echo "$dict") > "$dict_file"

	cp "${fragment_dir}/${rules_file_name}" "${rules_file}"
}

spell_check_file()
{
	local file="$1"

	[ -z "$file" ] && die "need file to check"
	[ -e "$file" ] || die "file does not exist: '$file'"

	[ -e "$dict_file" ] || make_dictionary

	info "Spell checking file '$file'"

	# Determine the pandoc input format.
	local pandoc_input_fmts
	local pandoc_input_fmt

	local pandoc_input_fmts=$(pandoc --list-input-formats 2>/dev/null || true)

	if [ -z "$pandoc_input_fmts" ]
	then
		# We're using a very old version of pandoc that doesn't
		# support listing its available input formats, so
		# specify a default.
		pandoc_input_fmt="markdown_github"
	else
		# Pandoc has multiple names for the gfm parser so find one of them
		pandoc_input_fmt=$(echo "$pandoc_input_fmts" |\
			grep -E "gfm|github" |\
			head -1 || true)
	fi

	[ -z "$pandoc_input_fmt" ] && die "cannot find usable pandoc input format"

	local stripped_doc

	local pandoc_doc
	local utf8_free_doc
	local pre_hunspell_doc
	local hunspell_results
	local final_results

	# First strip out all code blocks and convert all
	# "quoted apostrophe's" ('\'') back into a single apostrophe.
	stripped_doc=$("$strip_cmd" -i "$file" -)

	# Next, convert the remainder it into plain text to remove the
	# remaining markdown syntax.
	#
	# Before pandoc gets hold of it:
	#
	# - Replace pipes with spaces. This
	#   fixes an issue with old versions of pandoc (Ubuntu 16.04)
	#   which completely mangle tables into nonsense.
	#
	# - Remove empty reference links.
	#
	#   For example, this markdown
	#
	#       blah [`qemu-lite`][qemu-lite] blah.
	#         :
	#       [qemu-lite]: https://...
	#
	#   Gets converted into
	#
	#       blah [][qemu-lite] blah.
	#         :
	#       [qemu-lite]: https://...
	#
	#   And the empty set of square brackets confuses pandoc.
	#
	# After pandoc has processed the data, remove any remaining
	# "inline links" in this format:
	#
	#     [link name](#link-address)
	#
	# This is strictly only required for old versions of pandoc.

	pandoc_doc=$(echo "$stripped_doc" |\
		tr '|' ' '  |\
		sed 's/\[\]\[[^]]*\]//g' |\
		"$md_convert_tool" -f "${pandoc_input_fmt}" -t plain - |\
		sed 's/\[[^]]*\]([^\)]*)//g' || true)

	# Convert the file into "pure ASCII" by removing all awkward
	# Unicode characters that won't spell check.
	#
	# Necessary since pandoc is "clever" and will convert things like
	# GitHub's colon emojis (such as ":smile:") into the actual utf8
	# character where possible.
	utf8_free_doc=$(echo "$pandoc_doc" | iconv -c -f utf-8 -t ascii)

	# Next, perform the following simplifications:
	#
	# - Remove URLs.
	# - Remove email addresses.
	# - Replace most punctuation symbols with a space
	#   (excluding a dash (aka hyphen!)
	# - Carefully remove non-hyphen dashes.
	# - Remove GitHub @userids.
	pre_hunspell_doc=$(echo "$utf8_free_doc" |\
		sed 's,https*://[^[:space:]()][^[:space:]()]*,,g' |\
		sed -r 's/[a-zA-Z0-9.-]+@[a-zA-Z0-9-]+\.[a-zA-Z0-9.-]+//g' |\
		tr '[,\[\]()\*\\/\|=]' ' ' |\
		sed -e 's/^ *-//g' -e 's/- $//g' -e 's/ -//g' |\
		sed 's/@[a-zA-Z0-9][a-zA-Z0-9]*\b//g')

	# Call the spell checker
	hunspell_results=$(echo "$pre_hunspell_doc" | $spell_check_cmd -d "${dict_languages}")

	# Finally, post-process the hunspell output:
	#
	# - Parse the output to ignore:
	#   - Hunspell banner.
	#   - Correctly spelt words (lines starting with '*', '+' or '-').
	#   - All words containing numbers (like "100MB").
	#   - All words that appear to be acronymns / Abbreviations
	#     (atleast two upper-case letters and which may be plural or
	#     possessive).
	#   - All words that appear to be numbers.
	#   - All possessives and the dreaded isolated "'s" which occurs
	#     for input like this:
	#
	#         `kata-shim`'s
	#
	#     which gets converted by $strip_cmd into simply:
	#
	#         's
	#
	# - Sort output.

	final_results=$(echo "$hunspell_results" |\
		grep -Evi "(ispell|hunspell)" |\
		grep -Ev '^(\*|\+|-)' |\
		grep -Evi "^(&|#) [^ ]*[0-9][^ ]*" |\
		grep -Ev "^. [A-Z][A-Z][A-Z]*(s|'s)*" |\
		grep -Ev "^. 's" |\
		sort -u || true)

	local line
	local incorrects
	local near_misses

	near_misses=$(echo "$final_results" | grep '^&' || true)
	incorrects=$(echo "$final_results" | grep '^\#' | awk '{print $2}' || true)

	local -i failed=0

	[ -n "$near_misses" ] && failed+=1
	[ -n "$incorrects" ] && failed+=1

	echo "$near_misses" | while read -r line
	do
		[ "$line" = "" ] && continue

		local word
		local possibles

		word=$(echo "$line" | awk '{print $2}')
		possibles=$(echo "$line" | cut -d: -f2- | sed 's/^ *//g')

		warn "Word '${word}': did you mean one of the following?: ${possibles}"
	done

	local incorrect
	for incorrect in $incorrects
	do
		warn "Incorrect word: '$incorrect'"
	done

	[ "$failed" -gt 0 ] && die "Spell check failed for file: '$file'"

	info "Spell check successful for file: '$file'"
}

delete_dictionary()
{
	rm -f "${KATA_DICT_FILE}" "${KATA_RULES_FILE}"
}

setup()
{
	local cmd

	for cmd in "$spell_check_cmd" "$md_convert_tool"
	do
		command -v "$cmd" &>/dev/null || die "Need $cmd command"
	done
}

usage()
{
	cat <<-EOF
	Usage: ${script_name} <command> [arguments]

	Description: Spell-checking utility.

	Commands:

	  check <file> : Spell check the specified file
	                 (implies 'make-dict').
	  delete-dict  : Delete the dictionary.
	  help         : Show this usage.
	  make-dict    : Create the dictionary.
EOF
}

main()
{
	setup

	[ -z "${1:-}" ] && usage && echo && die "need command"

	case "$1" in
		check) shift && spell_check_file "$1" ;;
		delete-dict) delete_dictionary ;;
		help|-h|--help) usage && exit 0 ;;
		make-dict) make_dictionary ;;
		*) die "invalid command: '$1'" ;;
	esac
}

main "$@"
