#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

# Signify to all scripts that they are running in a CI environment
[ -z "${KATA_DEV_MODE}" ] && export CI=true

# Need the repo to know which tests to run.
export kata_repo="$1"

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

# Get the repository and move to the correct commit
go get -u -d "${kata_repo}" || true

# Get the tests repo in case this repo was cloned using
# `git clone` and it is not in the $GOPATH
if [ "${kata_repo}" != "${tests_repo}" ]
then
	go get -u -d "${tests_repo}" || true
fi

pushd "${GOPATH}/src/${kata_repo}"

pr_number=

# $ghprbPullId and $ghprbTargetBranch are variables from
# the Jenkins GithubPullRequestBuilder Plugin
[ "${ghprbPullId}" ] && [ "${ghprbTargetBranch}" ] && pr_number="${ghprbPullId}"

if [ -n "$pr_number" ]
then
	pr_branch="PR_${pr_number}"

	# Create a separate branch for the PR. This is required to allow
	# checkcommits to be able to determine how the PR differs from
	# "master".
	git fetch origin "pull/${pr_number}/head:${pr_branch}"
	git checkout "${pr_branch}"
	git rebase "origin/${ghprbTargetBranch}"

	# As we currently have CC and runv runtimes as git submodules
	# we need to update the submodules in order to get
	# the correct changes.
	# This condition should be removed when we have one runtime.
	[ -f .gitmodules ] && git submodule update
else
	# Othewise we test the master branch
	git fetch origin && git checkout master && git reset --hard origin/master
fi

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
.ci/setup.sh

# Run the static analysis tools
if [ -z "${METRICS_CI}" ]
then
	.ci/static-checks.sh
fi

if [ -n "$pr_number" ]
then
	# Now that checkcommits has run, move the PR commits into the master
	# branch before running the tests. Having the commits in "master" is
	# required to ensure coveralls works.
	git checkout master
	git reset --hard "$pr_branch"
	git branch -D "$pr_branch"
fi

if [ -z "${METRICS_CI}" ]
then
	if [ "${kata_repo}" != "${tests_repo}" ]
	then
		echo "INFO: Running unit tests for repo $kata_repo"
		make test
	fi

	# Run integration tests
	#
	# Note: this will run all classes of tests for ${tests_repo}.
	.ci/run.sh

	# Code coverage
	bash <(curl -s https://codecov.io/bash)
fi

popd
