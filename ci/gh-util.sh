#!/bin/bash

# Copyright (c) 2020 Intel Corporation
# Copyright (c) 2024 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o errtrace
set -o nounset
set -o pipefail

[ -n "${DEBUG:-}" ] && set -o xtrace

script_name=${0##*/}

#---------------------------------------------------------------------

die()
{
    echo >&2 "$*"
    exit 1
}

usage()
{
    cat <<EOF
Usage: $script_name [OPTIONS] [command] [arguments]

Description: Utility to expand the abilities of the GitHub CLI tool, gh.

Command descriptions:

  list-issues-for-pr     List issues linked to a PR.
  list-labels-for-issue  List labels, in json format for an issue

Commands and arguments:

  list-issues-for-pr <pr>
  list-labels-for-issue <issue>

Options:

 -h                 Show this help statement.
 -r <owner/repo>    Optional <org/repo> specification. Default: 'kata-containers/kata-containers'

Examples:

- List issues for a Pull Request 123 in kata-containers/kata-containers repo

  $ $script_name list-issues-for-pr 123
EOF
}

list_issues_for_pr()
{
    local pr="${1:-}"
    local repo="${2:-kata-containers/kata-containers}"

    [ -z "$pr" ] && die "need PR"

    local commits=$(gh pr view ${pr} --repo ${repo} --json commits --jq .commits[].messageBody)

    [ -z "$commits" ] && die "cannot determine commits for PR $pr"

    # Extract the issue number(s) from the commits.
    #
    # This needs to be careful to take account of lines like this:
    #
    # fixes 99
    # fixes: 77
    # fixes #123.
    # Fixes: #1, #234, #5678.
    #
    # Note the exclusion of lines starting with whitespace which is
    # specifically to ignore vendored git log comments, which are whitespace
    # indented and in the format:
    #
    #     "<git-commit> <git-commit-msg>"
    #
    local issues=$(echo "$commits" |\
        grep -v -E "^( |	)" |\
        grep -i -E "fixes:* *(#*[0-9][0-9]*)" |\
        tr ' ' '\n' |\
        grep "[0-9][0-9]*" |\
        sed 's/[.,\#]//g' |\
        sort -nu || true)

    [ -z "$issues" ] && die "cannot determine issues for PR $pr"

    echo "# Issues linked to PR"
    echo "#"
    echo "# Fields: issue_number"

    local issue
    echo "$issues"|while read issue
    do
        printf "%s\n" "$issue"
    done
}

list_labels_for_issue()
{
    local issue="${1:-}"

    [ -z "$issue" ] && die "need issue number"

    local labels=$(gh issue view ${issue} --repo kata-containers/kata-containers --json labels)

    [ -z "$labels" ] && die "cannot determine labels for issue $issue"

    printf "$labels"
}

setup()
{
    for cmd in gh jq
    do
        command -v "$cmd" &>/dev/null || die "need command: $cmd"
    done
}

handle_args()
{
    setup

    local show_all="false"
    local opt

    while getopts "ahr:" opt "$@"
    do
        case "$opt" in
            a) show_all="true" ;;
            h) usage && exit 0 ;;
            r) repo="${OPTARG}" ;;
        esac
    done

    shift $(($OPTIND - 1))

    local repo="${repo:-kata-containers/kata-containers}"
    local cmd="${1:-}"

    case "$cmd" in
        list-issues-for-pr) ;;
        list-labels-for-issue) ;;

        "") usage && exit 0 ;;
        *) die "invalid command: '$cmd'" ;;
    esac

    # Consume the command name
    shift

    local issue=""
    local pr=""

    case "$cmd" in
        list-issues-for-pr)
            pr="${1:-}"

            list_issues_for_pr "$pr" "${repo}"
            ;;

        list-labels-for-issue)
            issue="${1:-}"

            list_labels_for_issue "$issue"
            ;;

        *) die "impossible situation: cmd: '$cmd'" ;;
    esac

    exit 0
}

main()
{
    handle_args "$@"
}

main "$@"
