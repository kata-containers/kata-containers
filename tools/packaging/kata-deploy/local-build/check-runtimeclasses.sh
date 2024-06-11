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

script_dir=$(dirname "$(readlink -f "$0")")

# Explicitly export LC_ALL to ensure `sort` sorting is expected on
# different environments.
export LC_ALL=C

readonly generated_file="resultingRuntimeClasses.yaml"
readonly original_file="kata-runtimeClasses.yaml"

pushd "${script_dir}/../runtimeclasses/"
rm -f $generated_file

echo "::group::Combine runtime classes"
for runtimeClass in $(find . -type f \( -name "*.yaml" -and -not -name "kata-runtimeClasses.yaml" \) | sort); do
    echo "Adding ${runtimeClass} to the $generated_file"
    cat "${runtimeClass}" >> $generated_file;
done
echo "::endgroup::"

echo "::group::Displaying the content of $generated_file"
cat $generated_file
echo "::endgroup::"

echo ""
echo "::group::Displaying the content of $original_file"
cat $original_file
echo "::endgroup::"

echo ""
if ! diff $generated_file $original_file; then
    echo ""
    echo "CHECKER FAILED: files $(pwd)/$generated_file (GENERATED) and $(pwd)/$original_file differ"
    exit 1
fi
echo "CHECKER PASSED"