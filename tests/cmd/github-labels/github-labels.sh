#!/bin/bash
#
# Copyright (c) 2019-2023 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

# Description: Generate the combined GitHub labels database for the
#   specified repository.

set -e

script_name=${0##*/}

source "/etc/os-release" || "source /usr/lib/os-release"

self_dir=$(dirname "$(readlink -f "$0")")
yqdir="${self_dir}/../../../ci"
cidir="${self_dir}/../.."
source "${cidir}/common.bash"

typeset -r labels_file="labels.yaml"
typeset -r labels_template="${labels_file}.in"

typeset -r master_labels_file="${self_dir}/${labels_file}"
typeset -r master_labels_template="${self_dir}/${labels_template}"

# The GitHub labels API requires a colour for each label so
# default to a white background.
typeset -r default_color="ffffff"

need_yq() {
	# install yq if not exist
	${yqdir}/install_yq.sh

	command -v yq &>/dev/null || \
		die 'yq command not found. Ensure "$GOPATH/bin" is in your $PATH.'
}

merge_yaml()
{
	local -r file1="$1"
	local -r file2="$2"
	local -r out="$3"

	[ -n "$file1" ] || die "need 1st file"
	[ -n "$file2" ] || die "need 2nd file"
	[ -n "$out" ] || die "need output file"

	need_yq
  yq eval-all '. as $item ireduce ({}; . *+ $item)' "$file1" "$file2" > "$out"
}

check_yaml()
{
	local -r file="$1"

	[ -n "$file" ] || die "need file to check"

	need_yq
	yq "$file" >/dev/null

	[ -z "$(command -v yamllint)" ] && die "need yamllint installed"

	# Deal with different versions of the tool
	local opts=""
	local has_strict_opt=$(yamllint --help 2>&1|grep -- --strict)

	[ -n "$has_strict_opt" ] && opts+="--strict"

	yamllint $opts "$file"
}

# Expand the variables in the labels database.
generate_yaml()
{
	local repo="$1"
	local template="$2"
	local out="$3"

	[ -n "$repo" ] || die "need repo"
	[ -n "$template" ] || die "need template"
	[ -n "$out" ] || die "need output file"

	local repo_slug=$(echo "${repo}"|sed 's!github.com/!!g')

	sed \
		-e "s|REPO_SLUG|${repo_slug}|g" \
		-e "s|DEFAULT_COLOUR|${default_color}|g" \
		"$template" > "$out"

	check_yaml "$out"
}

cmd_generate()
{
	local repo="$1"
	local out_file="$2"

	[ -n "$repo" ] || die "need repo"
	[ -n "$out_file" ] || die "need output file"

	# Create the master database from the template
	generate_yaml \
		"${repo}" \
		"${master_labels_template}" \
		"${master_labels_file}"

	local -r repo_labels_template="${GOPATH}/src/${repo}/${labels_template}"
	local -r repo_labels_file="${GOPATH}/src/${repo}/${labels_file}"

	# Check for a repo-specific set of labels
	if [ -e "${repo_labels_template}" ]; then
		info "Found repo-specific labels database"

		# Generate repo-specific labels from template
		generate_yaml \
			"${repo}" \
			"${repo_labels_template}" \
			"${repo_labels_file}"

		# Combine the two databases
		tmp=$(mktemp)

		merge_yaml \
			"${master_labels_file}" \
			"${repo_labels_file}" \
			"${tmp}"

		mv "${tmp}" "${out_file}"
	else
		info "No repo-specific labels database"
		cp "${master_labels_file}" "${out_file}"
	fi


	info "Generated labels database ${out_file}"

	# Perform checks
	kata-github-labels check "${out_file}"
}

usage()
{
	cat <<EOF
Usage: ${script_name} help
       ${script_name} generate <repo-name> <output-file>

Examples:

  # Generate combined labels database for runtime repo and write to
  # specified file
  \$ ${script_name} generate github.com/kata-containers/kata-containers /tmp/out.yaml

EOF
}

main()
{
	case "$1" in
		generate)
			shift
			cmd_generate "$@"
			;;

		help|"")
			usage
			exit 0
			;;

		*)
			die "Invalid command: '$1'"
			;;
	esac
}

main "$@"
