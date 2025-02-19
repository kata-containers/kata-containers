#!/usr/bin/env bash
#
# Copyright (c) 2024 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

[ -z "${DEBUG}" ] || set -x
set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

script_dir=$(dirname "$(readlink -f "$0")")
install_libseccomp_script_src="${script_dir}/../../../../ci/install_libseccomp.sh"
install_libseccomp_script_dest="${script_dir}/../../static-build/$1/install_libseccomp.sh"

cp "${install_libseccomp_script_src}" "${install_libseccomp_script_dest}"

# We don't have to import any other file, as we're passing
# the env vars needed for installing libseccomp and gperf.
sed -i -e '/^source.*$/d' ${install_libseccomp_script_dest}
