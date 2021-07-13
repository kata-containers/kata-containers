#!/bin/bash
#
# Copyright (c) 2018-2021 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -o errexit
set -o nounset
set -o pipefail
set -o errtrace

yq_path="/usr/local/bin/yq"
yq_pkg="github.com/mikefarah/yq"
goos="linux"
case "$(uname -m)" in
	aarch64) goarch="arm64";;
	ppc64le) goarch="ppc64le";;
	s390x) goarch="s390x";;
	x86_64) goarch="amd64";;
	*) echo >&2 "ERROR: unsupported architecture: $(uname -m)"; exit 1;;
esac
yq_version=3.4.1
yq_url="https://${yq_pkg}/releases/download/${yq_version}/yq_${goos}_${goarch}"
curl -o "${yq_path}" -LSsf "${yq_url}"
chmod +x "${yq_path}"
