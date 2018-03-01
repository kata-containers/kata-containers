#!/bin/bash
#
# Copyright (c) 2018 Intel Corporation
#
# SPDX-License-Identifier: Apache-2.0
#

set -e

# Need the repo to know which tests to run.
kata_repo="$1"

tests_repo="${tests_repo:-github.com/kata-containers/tests}"

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
else
	# Othewise we test the master branch
	git fetch origin && git checkout master && git reset --hard origin/master
fi

# Run the static analysis tools
.ci/static-checks.sh

# Setup Kata Containers Environment
.ci/setup.sh

if [ -n "$pr_number" ]
then
	# Now that checkcommits has run, move the PR commits into the master
	# branch before running the tests. Having the commits in "master" is
	# required to ensure coveralls works.
	git checkout master
	git reset --hard "$pr_branch"
	git branch -D "$pr_branch"
fi

# Run integration tests
.ci/run.sh

popd
