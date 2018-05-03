#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

cidir=$(dirname "$0")
source /etc/os-release
source "${cidir}/lib.sh"

if [ "$ID" != "fedora" ] && [ "$CI" == true ]; then
	echo "Skip Openshift Installation on $ID"
	echo "CI only runs openshift tests on fedora"
	exit
fi

openshift_origin_version=$(get_version "externals.openshift.version")
openshift_origin_commit=$(get_version "externals.openshift.commit")

echo "Install Skopeo"
skopeo_repo="github.com/projectatomic/skopeo"
go get -d "$skopeo_repo" || true
pushd "$GOPATH/src/$skopeo_repo"
make binary-local
sudo -E PATH=$PATH make install-binary
popd

echo "Install Openshift Origin"
openshift_repo="github.com/openshift/origin"
openshift_tarball="openshift-origin-server-${openshift_origin_version}-${openshift_origin_commit}-linux-64bit.tar.gz"
openshift_dir="${openshift_tarball/.tar.gz/}"
openshift_url="https://${openshift_repo}/releases/download/${openshift_origin_version}/${openshift_tarball}"

curl -L -O "$openshift_url"
tar -xf "$openshift_tarball"
sudo install ${openshift_dir}/{openshift,oc,oadm} /usr/bin
rm -rf "$openshift_dir" "${openshift_tarball}"

echo "Openshift installed successfully"
