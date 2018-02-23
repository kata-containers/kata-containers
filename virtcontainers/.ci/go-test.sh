# Copyright (c) 2017 Intel Corporation

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

#!/bin/bash

set -e

test_packages=$(go list ./... | grep -v vendor)
test_ldflags="-X github.com/containers/virtcontainers/pkg/mock.DefaultMockCCShimBinPath=$1 \
		-X github.com/containers/virtcontainers/pkg/mock.DefaultMockKataShimBinPath=$2 \
		-X github.com/containers/virtcontainers/pkg/mock.DefaultMockHookBinPath=$3"
echo "Run go test and generate coverage:"
for pkg in $test_packages; do
	if [ "$pkg" = "github.com/containers/virtcontainers" ]; then
		sudo env GOPATH=$GOPATH GOROOT=$GOROOT PATH=$PATH go test -ldflags "$test_ldflags" -cover -coverprofile=profile.cov $pkg
	else
		sudo env GOPATH=$GOPATH GOROOT=$GOROOT PATH=$PATH go test -ldflags "$test_ldflags" -cover $pkg
	fi
done
