#!/usr/bin/env bash
#
# Copyright 2021 Fabiano Fidêncio
#
# SPDX-License-Identifier: Apache-2.0
#

KATA_DEPLOY_DIR="`dirname $0`/../"
KATA_DEPLOY_ARTIFACT="$1"

echo "Copying $KATA_DEPLOY_ARTIFACT to $KATA_DEPLOY_DIR"
cp $KATA_DEPLOY_ARTIFACT $KATA_DEPLOY_DIR

pushd $KATA_DEPLOY_DIR

IMAGE_TAG="quay.io/kata-containers/kata-deploy-cc:v0"

echo "Building the image"
docker build --tag $IMAGE_TAG .

echo "Pushing the image to quay.io"
docker push $IMAGE_TAG

popd
