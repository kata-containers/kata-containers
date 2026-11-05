#!/usr/bin/env bash
# Copyright (c) 2026 NVIDIA Corporation
# SPDX-License-Identifier: Apache-2.0

set -euo pipefail

[[ $# == 1 ]] || { echo "Usage: $0 ASSETS" >&2; exit 1; }
assets=$(realpath "$1")
script_dir=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd "${script_dir}/../../.." && pwd)

bash "${repo}/tools/packaging/kata-deploy/local-build/tests/substrate-artifacts.sh"
bash "${script_dir}/test-stage-artifacts.sh" "${assets}"
bash "${script_dir}/test-verify-artifacts.sh"
bash "${script_dir}/test-e2e-ci.sh"
