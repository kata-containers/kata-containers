#!/bin/bash
# Copyright (c) 2019 Intel Corporation
# Copyright (c) 2020 Ant Group
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o pipefail


main() {
    artifact_stage=${1:-}
    artifact=$(echo  ${artifact_stage} | sed -n -e 's/^install_//p' | sed -r 's/_/-/g')
    if [ -z "${artifact}" ]; then
        "Scripts needs artifact name to build"
        exit 1
    fi

    tag=$(echo $GITHUB_REF | cut -d/ -f3-)
    pushd $GITHUB_WORKSPACE/tools/packaging
    git checkout $tag
    ./scripts/gen_versions_txt.sh $tag
    popd

    pushd $GITHUB_WORKSPACE/tools/packaging/release
    source ./kata-deploy-binaries.sh
    ${artifact_stage} $tag
    popd

    mv $GITHUB_WORKSPACE/tools/packaging/release/kata-static-${artifact}.tar.gz .
}

main $@
