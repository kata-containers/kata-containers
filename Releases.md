* [Introduction](#introduction)
* [Versioning](#versioning)
* [Tagging repositories](#tagging-repositories)
* [Components](#components)
* [Release checklist](#release-checklist)
* [Release process](#release-process)

## Introduction

This document provides details about Kata Containers releases.

## Versioning

The Kata Containers project uses [semantic versioning](http://semver.org/) for all releases. Semantic versions are comprised of three fields in the form:

```
MAJOR.MINOR.PATCH
```

For examples: `1.0.0`, `1.0.0-rc.5`, and `99.123.77+foo.bar.baz.5`.

Semantic versioning is used since the version number is able to convey clear information about how a new version relates to the previous version. For example, semantic versioning can also provide assurances to allow users to know when they must upgrade compared with when they might want to upgrade:

- When `PATCH` increases, the new release contains important **security fixes**
  and an upgrade is recommended.

  The patch field can contain extra details after the number. Dashes denote pre-release versions. `1.0.0-rc.5` in the example denotes the fifth release candidate for release `1.0.0`. Plus signs denote other details. In our example, `+foo.bar.baz.5` provides additional information regarding release `99.123.77` in the previous example.

- When `MINOR` increases, the new release adds **new features** but *without
  changing the existing behavior*.

- When `MAJOR` increases, the new release adds **new features, bug fixes, or
  both** and which *changes the behavior from the previous release* (incompatible with previous releases).

  A major release will also likely require a change of the container manager version used, for example Docker\*. Please refer to the release notes for further details.

## Tagging repositories

To create a signed and annotated tag for a repository, first ensure that `git(1)` is configured to use your `gpg(1)` key:

```
$ git config --global user.signingkey $gpg_key_id
```

To create a signed and annotated tag:

```
$ git tag -as $tag
```

The tag name (`$tag` in the previous example) must conform to the [versioning](#versioning) requirements (e.g. `1.0.0-rc2`).

The annotation text must conform to the usual [patch format rules](https://github.com/kata-containers/community/blob/master/CONTRIBUTING.md#patch-format). Specifically:

- The subsystem must be "`release: $tag`".
- The body of the message must contain details of changes in the release in `git-shortlog(1)` format.

## Components

A new release will result in all Kata components being given a new [version](#versioning), even if no changes were made to that component since the last version. The version for a release is **identical** for all  components.

This strategy allows diagnostic tools such as `kata-runtime kata-env` to record full version details of all components to help with problem determination.

Note that although hypervisor and guest kernel both have versions, these are not updated for new releases as they are not core components developed by the Kata community.

## Release checklist

The detailed steps to follow to create a new release are specified in the [Release Checklist](Release-Checklist.md).

## Release process

The Release Owner must follow the following process, which is designed to ensure clarity, quality, stability, and auditability of each release:

- Raise a [new GitHub issue in the `kata-containers` repository](https://github.com/kata-containers/kata-containers/issues/new) and assign to themselves.

  This issue is used to track the progress of the release with maximum visibility.

- Paste the release checklist into the issue.

- Follow the instructions in the release Checklist and "check" each box in the issue as they are completed.

  This is useful for tracking so that the stage of the release is visible to all interested parties.

- Once all steps are complete, close the issue.
