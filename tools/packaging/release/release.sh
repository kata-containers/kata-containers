#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

[ -n "${DEBUG:-}" ] && set -o xtrace

this_script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
repo_root_dir="$(cd "$this_script_dir/../../../" && pwd)"

function _die()
{
	echo >&2 "ERROR: $*"
	exit 1
}

function _info()
{
	echo "INFO: $*"
}

function main()
{
	action="${1:-}"
	_info "DO NOT USE this script, it does nothing!"

	case "${action}" in
		*) >&2 _die "Invalid argument" ;;
	esac
}

main "$@"
