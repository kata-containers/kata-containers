#!/bin/bash
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
#set -o nounset
set -o pipefail


main() {
	artifact_stage=${1:-}
	artifact=$(echo  ${artifact_stage} | sed -n -e 's/^install_//p')
	if [ -z "${artifact}" ]; then
		"Scripts needs artifact name to build"
		exit 1
	fi
	
	github_ref=${2:-}
	if [ -z "${github_ref}" ]; then
		"Scripts needs github reference to build"
		exit 1
	fi

	tag=`echo ${github_ref} | cut -d/ -f3-`
	export GOPATH=$HOME/go

	echo tag: "$tag"		
	echo artifact "$artifact"
	echo artifact_stage "$artifact_stage"
	tag="1.9.0-rc0"

        export GOPATH=$HOME/go
        go get github.com/kata-containers/packaging || true
        pushd $GOPATH/src/github.com/kata-containers/packaging/release >>/dev/null
	git checkout $tag
        #git checkout master
        pushd ../obs-packaging
        echo "Running gen_versions_txt.sh with tag $tag"
	./gen_versions_txt.sh $tag
        popd

        ls -la
        echo $pwd  
        source ./kata-deploy-binaries.sh
	#${artifact_stage}
	install_kernel
        popd		
}

main $@
