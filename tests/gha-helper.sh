#!/usr/bin/env bash
#
# Copyright (c) 2025 IBM Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

[[ -n "${DEBUG:-}" ]] && set -o xtrace

function trigger_and_check_workflow() {
    workflow=$1
    ref=$2
    sha=$3
    input_json=$4

    trigger_workflow "${workflow}" "${ref}" "${sha}" "${input_json}"
    wait_for_workflow_result "${workflow}" "${sha}"
}

function trigger_workflow() {
    workflow=$1
    ref=$2
    sha=$3
    input_json=$4

    echo "${input_json}" | gh workflow run "${workflow}" --ref "${ref}" --json

    local max_tries=5
	local interval=15
	local i=0
    echo "::group::waiting"
    while true; do
        url=$(gh run list --workflow="${workflow}" --json headSha,url \
            --jq '.[] | select(.headSha == "'"${sha}"'") | .url')
        [[ -n "${url}" ]] && break
		i=$((i + 1))
		[ ${i} -lt ${max_tries} ] && echo "url of workflow not found, retrying in ${interval} seconds" 1>&2 || break
		sleep "${interval}"
	done
    echo "::endgroup::"
    echo "Triggered workflow: ${url}"
}

function wait_for_workflow_result() {
    workflow=$1
    sha=$2

    local max_tries=60
	local interval=120
	local i=0
            echo "::group::waiting"
    while true; do
        conclusion=$(gh run list --workflow="${workflow}" --json headSha,conclusion \
            --jq '.[] | select(.headSha == "'"${sha}"'") | .conclusion')

        case "${conclusion}" in
            "success") echo "::endgroup::\nJob finished successfully" && exit 0;;
            "cancelled") echo "::endgroup::\nJJob cancelled" && exit 4;;
            "failure") echo "::endgroup::\nJJob failed" && exit 8;;
            *) ;;
        esac

		i=$((i + 1))
		[ ${i} -lt ${max_tries} ] && echo "conclusion of workflow is ${conclusion}, retrying in ${interval} seconds" 1>&2 || break
		sleep "${interval}"
	done
    echo "Waiting for the workflow to succeed timed out"
    exit 16
}

function main() {
    action="${1:-}"
    case "${action}" in
        trigger-workflow) trigger_workflow "${@:2}";;
        trigger-and-check-workflow) trigger_and_check_workflow "${@:2}";;
        wait-for-workflow-result) wait_for_workflow_result "${@:2}";;
        *) >&2 echo "Invalid argument"; exit 2 ;;
    esac
}

main "$@"
