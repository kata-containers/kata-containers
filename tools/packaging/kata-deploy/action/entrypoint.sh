#!/usr/bin/env bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#
set -o errexit
set -o pipefail
set -o nounset

# This entrypoint expects an environment variable, PKG_SHA, to be
# within the container runtime. A default is provided in the Dockerfile,
# but we expect the caller to pass this into the container run (ie docker run -e PKG_SHA=foo ...)
echo "provided package reference: ${PKG_SHA}"

# Since this is the entrypoint for the container image, we know that the AKS and Kata setup/testing
# scripts are located at root.
source /setup-aks.sh
source /test-kata.sh 

trap destroy_aks EXIT

setup_aks
test_kata