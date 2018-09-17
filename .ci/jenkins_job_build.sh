#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

source "/etc/os-release" || source "/usr/lib/os-release"

# Signify to all scripts that they are running in a CI environment
[ -z "${KATA_DEV_MODE}" ] && export CI=true

# Need the repo to know which tests to run.
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

# Get the repository
mkdir -p $(dirname "${kata_repo_dir}")
[ -d "${kata_repo_dir}" ] || git clone "https://${kata_repo}.git" "${kata_repo_dir}"

# If CI running on bare-metal, a few clean-up work before walking into test repo
if [ "${BAREMETAL}" == true ]; then
	clean_up_script="${tests_repo_dir}/.ci/${arch}/clean_up_${arch}.sh"
	[ -f "${clean_up_script}" ] && source "${clean_up_script}"
fi

pushd "${kata_repo_dir}"

# Variables needed when we test a PR.
pr_number=
target_branch=

# Variable needed when a merge to a branch needs to be tested.
branch=

# $ghprbPullId and $ghprbTargetBranch are variables from
# the Jenkins GithubPullRequestBuilder Plugin
[ "${ghprbPullId}" ] && [ "${ghprbTargetBranch}" ] && export pr_number="${ghprbPullId}"



if [ -n "$pr_number" ]
then
	export target_branch="${ghprbTargetBranch:-master}"
	if [ "${kata_repo}" != "${tests_repo}" ]
	then
		# Use the correct branch for testing.
		# 'tests' repository branch should have the same name
		# of the kata repository branch where the change is
		# going to be merged.
		pushd "${tests_repo_dir}"
		git fetch origin && git checkout "${target_branch}"
		popd
	fi

	pr_branch="PR_${pr_number}"

	# Create a separate branch for the PR. This is required to allow
	# checkcommits to be able to determine how the PR differs from
	# the target branch.
	git fetch origin "pull/${pr_number}/head:${pr_branch}"
	git checkout "${pr_branch}"
	git rebase "origin/${target_branch}"

	# As we currently have CC and runv runtimes as git submodules
	# we need to update the submodules in order to get
	# the correct changes.
	# This condition should be removed when we have one runtime.
	[ -f .gitmodules ] && git submodule update
else
	# Othewise we test an specific branch
	# GIT_BRANCH env variable is set by the jenkins Github Plugin.
	[ -z "${GIT_BRANCH}" ] && echo >&2 "GIT_BRANCH is empty" && exit 1
	export branch="${GIT_BRANCH/*\//}"

	if [ "${kata_repo}" != "${tests_repo}" ]
	then
		# Use the correct branch for testing.
		# 'tests' repository branch should have the same name
		# as the kata repository branch that will be tested.
		pushd "${tests_repo_dir}"
		git fetch origin && git checkout "$branch"
		popd
	fi

	git fetch origin && git checkout "$branch" && git reset --hard "$GIT_BRANCH"
fi

# Install go after repository is cloned and checkout to PR
# This ensures:
# - We have latest changes in install_go.sh
# - We got get changes if versions.yaml changed.
${GOPATH}/src/${tests_repo}/.ci/install_go.sh -p

# Make sure runc is default runtime.
# This is needed in case a new image creation.
# See https://github.com/clearcontainers/osbuilder/issues/8
"${GOPATH}/src/${tests_repo}/cmd/container-manager/manage_ctr_mgr.sh" docker configure -r runc -f

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

if [ -n "$pr_number" ]
then
	# Now that checkcommits has run, move the PR commits into the target
	# branch before running the tests. Having the commits in the target branch
	# is required to ensure coveralls works.
	git checkout ${ghprbTargetBranch}
	git reset --hard "$pr_branch"
	git branch -D "$pr_branch"
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
