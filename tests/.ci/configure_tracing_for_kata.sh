#!/bin/bash
#
# Copyright (c) 2019 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

[ "$#" -eq 1 ] || die "Specify enable or disable"

kata_cfg_file=$(kata-runtime kata-env --json |jq '.Runtime | .Config | .Path' |cut -d\" -f2)

enable_tracing() {
	info "Enabling kata tracing on $kata_cfg_file"
	sudo crudini --set "$kata_cfg_file" shim.kata enable_tracing true
	sudo crudini --set "$kata_cfg_file" runtime enable_tracing true
	sudo crudini --set "$kata_cfg_file" runtime internetworking_model \"none\"
	sudo crudini --set "$kata_cfg_file" runtime disable_new_netns true
	sudo crudini --set "$kata_cfg_file" netmon enable_netmon false
}

disable_tracing() {
	info "Disabling kata tracing on $kata_cfg_file"
	sudo crudini --set "$kata_cfg_file" shim.kata enable_tracing false
	sudo crudini --set "$kata_cfg_file" runtime enable_tracing false
	sudo crudini --set "$kata_cfg_file" runtime internetworking_model \"macvtap\"
	sudo crudini --set "$kata_cfg_file" runtime disable_new_netns false
	sudo crudini --set "$kata_cfg_file" netmon enable_netmon false
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
