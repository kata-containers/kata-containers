#!/bin/bash

# Copyright (c) 2024 Microsoft Corporation
#
# SPDX-License-Identifier: Apache-2.0

set -e

kata_agent_apis_dir="$(dirname "$(readlink -f "$0")")"
source "${kata_agent_apis_dir}/../../common.bash"
source "${kata_agent_apis_dir}/setup_common.sh"

usage()
{
	cat <<EOF

Usage: $script_name [<command>]

Summary: Test agent ttrpc apis using agent-ctl tool.

Description: Test agent exposed ttrpc api endpoints using agent-ctl tool.
A number of variations of the inputs are used to test an inidividual api
to validate success & failure code paths.

Commands:

  help   - Show usage.

Notes:
  - Currently, the script *does not support* running individual agent api tests.

EOF
}

run_tests() {
    info "Running agent API tests"

    bats "${kata_agent_apis_dir}/api-tests"
}

main()
{
	local cmd="${1:-}"

	case "$cmd" in
		help|-h|-help|--help) usage; exit 0;;
	esac

	trap cleanup EXIT

	install_policy_doc

	try_and_remove_coco_attestation_procs

	setup_agent

	run_tests
}

main "$@"
