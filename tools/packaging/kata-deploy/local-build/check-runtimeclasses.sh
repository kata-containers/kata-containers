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

if ! command -v yq >/dev/null; then
    echo "ERROR: 'yq' not found. This script needs that tool" >&2
    exit 1
fi

pushd "${script_dir}/../runtimeclasses/"
rm -f $generated_file
runtimeClass_files="$(find . -type f \( -name "*.yaml" -and -not -name "kata-runtimeClasses.yaml" \) | sort)"

echo "::group::Combine runtime classes"
for runtimeClass in $runtimeClass_files; do
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
popd

#
# Checking the lists of SHIMS in the deployment files are up-to-dated.
#
for yaml in kata-deploy/base/kata-deploy.yaml \
            kata-cleanup/base/kata-cleanup.yaml;do
    # Get the current list of shims
    shim_list="$(yq '.spec.template.spec.containers[0].env[] | select(.name=="SHIMS").value' $yaml)"

    for file in $runtimeClass_files; do
        # shellcheck disable=2001
        shim="$(echo "$file" | sed 's/.*kata-\(.*\).yaml/\1/g')"

        # Ignore shims that shouldn't be in the list
        # shellcheck disable=2076
        [[  " qemu-se remote " =~ " $shim " ]] && continue

        # shellcheck disable=2076
        [[ " $shim_list " =~ " $shim " ]] && continue
        echo ""
        echo "CHECKER FAILED: '$shim' not found on list of SHIMS in $yaml"
        exit 1
    done
done

echo "CHECKER PASSED"