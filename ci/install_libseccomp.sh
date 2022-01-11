#!/bin/bash
#
# Copyright 2021 Sony Group Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit

cidir=$(dirname "$0")
source "${cidir}/lib.sh"

clone_tests_repo

source "${tests_repo_dir}/.ci/lib.sh"

# The following variables if set on the environment will change the behavior
# of gperf and libseccomp configure scripts, that may lead this script to
# fail. So let's ensure they are unset here.
unset PREFIX DESTDIR

arch=$(uname -m)
workdir="$(mktemp -d --tmpdir build-libseccomp.XXXXX)"

# Variables for libseccomp
# Currently, specify the libseccomp version directly without using `versions.yaml`
# because the current Snap workflow is incomplete.
# After solving the issue, replace this code by using the `versions.yaml`.
# libseccomp_version=$(get_version "externals.libseccomp.version")
# libseccomp_url=$(get_version "externals.libseccomp.url")
libseccomp_version="2.5.1"
libseccomp_url="https://github.com/seccomp/libseccomp"
libseccomp_tarball="libseccomp-${libseccomp_version}.tar.gz"
libseccomp_tarball_url="${libseccomp_url}/releases/download/v${libseccomp_version}/${libseccomp_tarball}"
cflags="-O2"

# Variables for gperf
# Currently, specify the gperf version directly without using `versions.yaml`
# because the current Snap workflow is incomplete.
# After solving the issue, replace this code by using the `versions.yaml`.
# gperf_version=$(get_version "externals.gperf.version")
# gperf_url=$(get_version "externals.gperf.url")
gperf_version="3.1"
gperf_url="https://ftp.gnu.org/gnu/gperf"
gperf_tarball="gperf-${gperf_version}.tar.gz"
gperf_tarball_url="${gperf_url}/${gperf_tarball}"

# We need to build the libseccomp library from sources to create a static library for the musl libc.
# However, ppc64le and s390x have no musl targets in Rust. Hence, we do not set cflags for the musl libc.
if ([ "${arch}" != "ppc64le" ] && [ "${arch}" != "s390x" ]); then
    # Set FORTIFY_SOURCE=1 because the musl-libc does not have some functions about FORTIFY_SOURCE=2
    cflags="-U_FORTIFY_SOURCE -D_FORTIFY_SOURCE=1 -O2"
fi

die() {
    msg="$*"
    echo "[Error] ${msg}" >&2
    exit 1
}

finish() {
    rm -rf "${workdir}"
}

trap finish EXIT

build_and_install_gperf() {
    echo "Build and install gperf version ${gperf_version}"
    mkdir -p "${gperf_install_dir}"
    curl -sLO "${gperf_tarball_url}"
    tar -xf "${gperf_tarball}"
    pushd "gperf-${gperf_version}"
    ./configure --prefix="${gperf_install_dir}"
    make
    make install
    export PATH=$PATH:"${gperf_install_dir}"/bin
    popd
    echo "Gperf installed successfully"
}

build_and_install_libseccomp() {
    echo "Build and install libseccomp version ${libseccomp_version}"
    mkdir -p "${libseccomp_install_dir}"
    curl -sLO "${libseccomp_tarball_url}"
    tar -xf "${libseccomp_tarball}"
    pushd "libseccomp-${libseccomp_version}"
    ./configure --prefix="${libseccomp_install_dir}" CFLAGS="${cflags}" --enable-static
    make
    make install
    popd
    echo "Libseccomp installed successfully"
}

main() {
    local libseccomp_install_dir="${1:-}"
    local gperf_install_dir="${2:-}"

    if [ -z "${libseccomp_install_dir}" ] || [ -z "${gperf_install_dir}" ]; then
        die "Usage: ${0} <libseccomp-install-dir> <gperf-install-dir>"
    fi

    pushd "$workdir"
    # gperf is required for building the libseccomp.
    build_and_install_gperf
    build_and_install_libseccomp
    popd
}

main "$@"
