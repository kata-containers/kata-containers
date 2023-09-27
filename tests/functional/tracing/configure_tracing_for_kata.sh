#!/bin/bash
#
# Copyright (c) 2019-2022 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

SCRIPT_PATH=$(dirname "$(readlink -f "$0")")
source "${SCRIPT_PATH}/../../common.bash"

[ "$#" -eq 1 ] || die "Specify enable or disable"

kata_cfg_file=$(kata-runtime kata-env --json |jq '.Runtime | .Config | .Path' |cut -d\" -f2)

enable_tracing() {
	info "Enabling kata tracing on $kata_cfg_file"
	sudo crudini --set "$kata_cfg_file" agent.kata enable_tracing true
	sudo crudini --set "$kata_cfg_file" runtime enable_tracing true
}

disable_tracing() {
	info "Disabling kata tracing on $kata_cfg_file"
	sudo crudini --set "$kata_cfg_file" agent.kata enable_tracing false
	sudo crudini --set "$kata_cfg_file" runtime enable_tracing false
}

main() {
	cmd="$1"
	case "$cmd" in
		enable ) enable_tracing ;;
		disable ) disable_tracing ;;
		*) die "invalid command: '$cmd'" ;;
	esac
}

main "$@"
