#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

source "/etc/os-release" || source "/usr/lib/os-release"

# Unit test issue for RHEL
unit_issue="https://github.com/kata-containers/runtime/issues/1517"

# Signify to all scripts that they are running in a CI environment
[ -z "${KATA_DEV_MODE}" ] && export CI=true

# Name of the repo that we are going to test
export kata_repo="$1"

echo "Setup env for kata repository: $kata_repo"

[ -z "$kata_repo" ] && echo >&2 "kata repo no provided" && exit 1

tests_repo="${tests_repo:-github.com/kata-containers/tests}"
runtime_repo="${runtime_repo:-github.com/kata-containers/runtime}"

# This script is intended to execute under Jenkins
# If we do not know where the Jenkins defined WORKSPACE area is
# then quit
if [ -z "${WORKSPACE}" ]
then
	echo "Jenkins WORKSPACE env var not set - exiting" >&2
	exit 1
fi

# Put our go area into the Jenkins job WORKSPACE tree
export GOPATH=${WORKSPACE}/go
mkdir -p "${GOPATH}"

# Export all environment variables needed.
export GOROOT="/usr/local/go"
export PATH=${GOPATH}/bin:/usr/local/go/bin:/usr/sbin:/sbin:${PATH}

kata_repo_dir="${GOPATH}/src/${kata_repo}"
tests_repo_dir="${GOPATH}/src/${tests_repo}"

# Get the tests repository
mkdir -p $(dirname "${tests_repo_dir}")
[ -d "${tests_repo_dir}" ] || git clone "https://${tests_repo}.git" "${tests_repo_dir}"

# Get the repository of the PR to be tested
mkdir -p $(dirname "${kata_repo_dir}")
[ -d "${kata_repo_dir}" ] || git clone "https://${kata_repo}.git" "${kata_repo_dir}"

# If CI running on bare-metal, a few clean-up work before walking into test repo
if [ "${BAREMETAL}" == true ]; then
	arch=$("${tests_repo_dir}/.ci/kata-arch.sh")
	echo "Looking for baremetal cleanup script for arch ${arch}"
	clean_up_script=("${tests_repo_dir}/.ci/${arch}/clean_up_${arch}.sh") || true
	if [ -f "${clean_up_script}" ]; then
		echo "Running baremetal cleanup script for arch ${arch}"
		tests_repo="${tests_repo}" "${clean_up_script}"
	else
		echo "No baremetal cleanup script for arch ${arch}"
	fi
fi

pushd "${kata_repo_dir}"

pr_number=
branch=

# $ghprbPullId and $ghprbTargetBranch are variables from
# the Jenkins GithubPullRequestBuilder Plugin
[ "${ghprbPullId}" ] && [ "${ghprbTargetBranch}" ] && export pr_number="${ghprbPullId}"

# Install go after repository is cloned and checkout to PR
# This ensures:
# - We have latest changes in install_go.sh
# - We got get changes if versions.yaml changed.
${GOPATH}/src/${tests_repo}/.ci/install_go.sh -p -f

if [ -n "$pr_number" ]; then
	export branch="${ghprbTargetBranch}"
	export pr_branch="PR_${pr_number}"
else
	export branch="${GIT_BRANCH/*\//}"
fi

# Resolve kata dependencies
"${GOPATH}/src/${tests_repo}/.ci/resolve-kata-dependencies.sh"

# Setup Kata Containers Environment
#
# - If the repo is "tests", this will call the script living in that repo
#   directly.
# - If the repo is not "tests", call the repo-specific script (which is
#   expected to call the script of the same name in the "tests" repo).
if [  "$CI_JOB" == "CRI_CONTAINERD_K8S" ]; then
	# This job only tests containerd + k8s
	export CRI_CONTAINERD="yes"
	export KUBERNETES="yes"
	export CRIO="no"
	export OPENSHIFT="no"
fi
.ci/setup.sh

# Run the static analysis tools
if [ -z "${METRICS_CI}" ]
then
	specific_branch=""
	# If not a PR, we are testing on stable or master branch.
	[ -z "$pr_number" ] && specific_branch="true"
	.ci/static-checks.sh "$kata_repo" "$specific_branch"
fi

# Now we have all the components installed, log that info before we
# run the tests.
if command -v kata-runtime; then
	echo "Logging kata-env information:"
	kata-runtime kata-env
else
	echo "WARN: Kata runtime is not installed"
fi

if [ -z "${METRICS_CI}" ]
then
	if [ "${kata_repo}" != "${tests_repo}" ]
	then
		[ "${ID}" == "rhel" ] && [ "${kata_repo}" == "${runtime_repo}" ] && skip "unit tests not working see: ${unit_issue}"

		if [ "${ID}" == "centos" ] && [ "${kata_repo}" == "${runtime_repo}" ]
		then
			echo "INFO: unit tests skipped for $kata_repo in $ID"
			echo "INFO: issue https://github.com/kata-containers/runtime/issues/228"
		else
			echo "INFO: Running unit tests for repo $kata_repo"
			make test
		fi
	fi

	# Run integration tests
	#
	# Note: this will run all classes of tests for ${tests_repo}.
	.ci/run.sh

	# Code coverage
	bash <(curl -s https://codecov.io/bash)
fi

popd
