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

# This script is used to update the virtcontainers code in the vendor
# directories of the proxy and runtime repositories.

set -e

proxy_repo="github.com/clearcontainers/proxy"
runtime_repo="github.com/clearcontainers/runtime"
virtcontainers_repo="github.com/containers/virtcontainers"

function apply_depends_on(){
	pushd "${GOPATH}/src/${virtcontainers_repo}"
	label_lines=$(git log --format=%s%b -n 1 | grep "Depends-on:" || true)

	if [ "${label_lines}" == "" ]; then
		return 0
	fi

	nb_lines=$(echo ${label_lines} | wc -l)

	for i in `seq 1 ${nb_lines}`
	do
		label_line=$(echo $label_lines | sed "${i}q;d")
		label_str=$(echo "${label_line}" | cut -d' ' -f2)
		repo=$(echo "${label_str}" | cut -d'#' -f1)
		pr_id=$(echo "${label_str}" | cut -d'#' -f2)

		if [ ! -d "${GOPATH}/src/${repo}" ]; then
			go get -d "$repo" || true
		fi

		pushd "${GOPATH}/src/${repo}"
		git fetch origin "pull/${pr_id}/head" && git checkout FETCH_HEAD && git rebase origin/master
		popd
	done

	popd
}

function install_dep(){
	go get -u github.com/golang/dep/cmd/dep
}

function update_repo(){
	if [ "${AUTHOR_REPO_GIT_URL}" ] && [ "${COMMIT_REVISION}" ]
	then
		repo="$1"
		if [ ! -d "${GOPATH}/src/${repo}" ]; then
			go get -d "$repo" || true
		fi

		pushd "${GOPATH}/src/${repo}"

		# Update Gopkg.toml
		cat >> Gopkg.toml <<EOF

[[override]]
  name = "${virtcontainers_repo}"
  source = "${AUTHOR_REPO_GIT_URL}"
  revision = "${COMMIT_REVISION}"
EOF

		# Update the whole vendoring
		dep ensure && dep ensure -update "${virtcontainers_repo}" && dep prune

		popd
	fi
}

apply_depends_on
install_dep
update_repo "${proxy_repo}"
update_repo "${runtime_repo}"
