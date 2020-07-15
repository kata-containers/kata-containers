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

See [the release documentation](../../../docs/Release-Process.md).

## Release tools

### `update-repository-version.sh`

This script creates a GitHub pull request (a.k.a PR) to change the version in
all the Kata repositories.

For more information on using the script, run the following:

```bash
$ ./update-repository-version.sh -h
```

### Update Kata projects to a new version

To update project version for Kata Containers, use the following:

```bash
$ make bump-kata-version NEW_VERSION=<new-version>
```

The makefile target `bump-kata-version` creates a GitHub pull request in the
kata-containers repository. The pull request is tested by the Kata CI to ensure the
entire project is working prior to the release. Next, the PR is approved and
merged by Kata Containers members.

### `tag_repos.sh`

After Kata Containers repository is updated with a new version, it needs to be
tagged.

The `tag_repos.sh` script is used to create tags for the Kata Containers repository.
The script creates an **annotated tag** for the new release version for the
following repositories:

- kata-containers
