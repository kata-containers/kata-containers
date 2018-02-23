#!/bin/bash
#
# Copyright (c) 2017 Intel Corporation
#
# Licensed under the Apache License, Version 2.0 (the "License");
# you may not use this file except in compliance with the License.
# You may obtain a copy of the License at
#
#      http://www.apache.org/licenses/LICENSE-2.0
#
# Unless required by applicable law or agreed to in writing, software
# distributed under the License is distributed on an "AS IS" BASIS,
# WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
# See the License for the specific language governing permissions and
# limitations under the License.
#

set -e

cidir=$(dirname "$0")
tests_repo="github.com/clearcontainers/tests"

# Clone Tests repository.
go get "$tests_repo"

tests_repo_dir="${GOPATH}/src/${tests_repo}"

echo "Update proxy and runtime vendoring"
sudo -E PATH=$PATH bash -c "${cidir}/update-vendoring.sh"

pushd "${tests_repo_dir}"
echo "Setup Clear Containers"
sudo -E PATH=$PATH bash -c ".ci/setup.sh"
popd

echo "Setup virtcontainers environment"
chronic sudo -E PATH=$PATH bash -c "${cidir}/../utils/virtcontainers-setup.sh"

echo "Install virtcontainers"
chronic make
chronic sudo make install
