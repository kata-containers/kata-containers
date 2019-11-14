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
	artifact=$(echo  ${artifact_stage} | sed -n -e 's/^install_//p' | sed -r 's/_/-/g')
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
	#tag="1.9.0-rc0"

        export GOPATH=$HOME/go
        #go get github.com/kata-containers/packaging || true
        go get github.com/amshinde/kata-packaging || true
        #pushd $GOPATH/src/github.com/kata-containers/packaging/release >>/dev/null
	pushd $GOPATH/src/github.com/amshinde/kata-packaging/release >>/dev/null
	git checkout $tag
        #git checkout master
        pushd ../obs-packaging
        echo "Running gen_versions_txt.sh with tag $tag"
	./gen_versions_txt.sh $tag
        popd

        echo Directory for running deploy script: $pwd  
        ls -la
        source ./kata-deploy-binaries.sh
	${artifact_stage}
	echo "Dir while doing final pop:" $pwd
        popd		

	echo "Done installing"
	echo PWD, should be top dir: $pwd
        ls -la
	#mv $HOME/go/src/github.com/kata-containers/packaging/release/kata-kernel.tar.gz .
	mv $HOME/go/src/github.com/amshinde/kata-packaging/release/kata-static-${artifact}.tar.gz .
}

main $@
