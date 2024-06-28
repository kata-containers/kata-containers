#!/bin/bash
#
# Copyright (c) 2022 Red Hat
#
# SPDX-License-Identifier: Apache-2.0
#

script_dir=$(dirname "$(readlink -f "$0")")
parent_dir=$(realpath "${script_dir}/../..")
cidir="${parent_dir}/ci"
source "${cidir}/../tests/common.bash"

cargo_deny_file="${script_dir}/action.yaml"

cat cargo-deny-skeleton.yaml.in > "${cargo_deny_file}"

changed_files_status=$(run_get_pr_changed_file_details)
changed_files_status=$(echo "$changed_files_status" | grep "Cargo\.toml$" || true)
changed_files=$(echo "$changed_files_status" | awk '{print $NF}' || true)

if [ -z "$changed_files" ]; then
  cat >> "${cargo_deny_file}" << EOF
    - run: echo "No Cargo.toml files to check"
      shell: bash
EOF
fi

for path in $changed_files
do
    cat >> "${cargo_deny_file}" << EOF

    - name: ${path}
      continue-on-error: true
      shell: bash
      run: |
        pushd $(dirname ${path})
        cargo deny check
        popd
EOF
done
