# Release information

* [Introduction](#introduction)
* [Create a Kata Containers release](#create-a-kata-containers-release)
* [Release tools](#release-tools)
  - [`update-repository-version.sh`](#update-repository-versionsh)
  - [Update Kata projects to a new version](#update-kata-projects-to-a-new-version)
  - [`tag_repos.sh`](#tag_repossh)

## Introduction

This directory contains information of the process and
tools used for creating Kata Containers releases.

## Create a Kata Containers release

See [the release documentation](https://github.com/kata-containers/documentation/blob/master/Release-Process.md).

## Release tools

### `update-repository-version.sh`

This script creates a GitHub pull request (a.k.a PR) to change the version in
all the Kata repositories.

For more information on using the script, run the following:

```bash
$ ./update-repository-version.sh -h
```

### Update Kata projects to a new version

Kata Containers is divided into multiple projects. With each release, all
project versions are updated to keep the version consistent.

To update all versions for all projects, use the following:

```bash
$ make bump-kata-version NEW_VERSION=<new-version>
```

The makefile target `bump-kata-version` creates a GitHub pull request in the
Kata repositories. These pull requests are tested by the Kata CI to ensure the
entire project is working prior to the release. Next, the PR is approved and
merged by Kata Containers members.

### `tag_repos.sh`

After all the Kata repositories are updated with a new version, they need to be
tagged.

The `tag_repos.sh` script is used to create tags for the Kata Containers
repositories. This script ensures that all the repositories are in the same
version (by checking the `VERSION` file).

The script creates an **annotated tag** for the new release version for the
following repositories:

- agent
- proxy
- runtime
- shim
- throttler

The script also tags the tests and osbuilder repositories to make it clear which
versions of these supporting repositories are used for the release.
