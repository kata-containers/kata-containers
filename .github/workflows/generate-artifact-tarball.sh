#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail


main() {
	artifact=${1:-}
        if [ -n "${artifact}" ]; then
		"Scripts needs artifact name to build"
		exit 1
	fi
        info "artifact name: ${artifact}"
	
	github_ref=${2:-}
	if [ -n "${github_ref}" ]; then
		"Scripts needs githun reference to build"
		exit 1
	fi

	tag=`echo ${github_ref} | cut -d/ -f3-`
	export GOPATH=$HOME/go

	echo tag: "tag"		
	echo artifact "artifact_name"
}

main $@
