#!/usr/bin/env bash
#
# Copyright (c) 2023-2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

pushd tools/packaging/kata-deploy/runtimeclasses/
echo "::group::Combine runtime classes"
for runtimeClass in `find . -type f \( -name "*.yaml" -and -not -name "kata-runtimeClasses.yaml" \) | sort`; do
    echo "Adding ${runtimeClass} to the resultingRuntimeClasses.yaml"
    cat ${runtimeClass} >> resultingRuntimeClasses.yaml;
done
echo "::endgroup::"
echo "::group::Displaying the content of resultingRuntimeClasses.yaml"
cat resultingRuntimeClasses.yaml
echo "::endgroup::"
echo ""
echo "::group::Displaying the content of kata-runtimeClasses.yaml"
cat kata-runtimeClasses.yaml
echo "::endgroup::"
echo ""
diff resultingRuntimeClasses.yaml kata-runtimeClasses.yaml