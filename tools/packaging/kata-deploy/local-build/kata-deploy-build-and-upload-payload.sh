#!/usr/bin/env bash
#
# Copyright 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0
#

KATA_DEPLOY_DIR="`dirname $0`/../../kata-deploy-cc"
KATA_DEPLOY_ARTIFACT="${1:-"kata-static.tar.xz"}"

echo "Copying $KATA_DEPLOY_ARTIFACT to $KATA_DEPLOY_DIR"
cp $KATA_DEPLOY_ARTIFACT $KATA_DEPLOY_DIR

pushd $KATA_DEPLOY_DIR

IMAGE_TAG="quay.io/confidential-containers/runtime-payload:kata-containers-$(git rev-parse HEAD)"

echo "Building the image"
docker build --tag $IMAGE_TAG .

echo "Pushing the image to quay.io"
docker push $IMAGE_TAG

popd
