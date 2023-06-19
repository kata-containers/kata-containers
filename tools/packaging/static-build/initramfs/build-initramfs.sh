#!/bin/bash
#
# Copyright (c) 2022 Intel
#
# SPDX-License-Identifier: Apache-2.0

set -o errexit
set -o nounset
set -o pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
source "${script_dir}/../../scripts/lib.sh"
install_dir="${1:-.}"

gen_init_cpio "${script_dir}/initramfs.list" | gzip -9 -n > "${install_dir}"/initramfs.cpio.gz
