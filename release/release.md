# How to do a Kata Containers Release

* [Introduction](#introduction)
* [Requirements](#requirements)
* [Release process](#release-process)

## Introduction

If you are reading this document, you might want to create a Kata Containers
Release.

The Kata Containers Release Process is defined in the following
[document][release-process-definition]. To simplify this process, we have
created a [Release Checklist][release-checklist]. Finally, to simplify the
Release Checklist process we have automated most of the process, this document
guides on how to use release scripts instead of do all the checklist manually.

## Requirements

- It is recommended to have at least 2 GB of free disk space to perform these tasks.

- [Go][install-go-kata]

- [hub](https://github.com/github/hub)

- OBS account with permissions on [`/home:katacontainers`](https://build.opensuse.org/project/subprojects/home:katacontainers)

- GitHub permissions to push tags and creates Releases in Kata repositories.

- GPG configured to sign git tags. https://help.github.com/articles/generating-a-new-gpg-key/

- You should configure your GitHub to use your ssh keys (to push to branches). See https://help.github.com/articles/adding-a-new-ssh-key-to-your-github-account/.
    * As an alternative, configure hub to push and fork with HTTPS, `git config --global hub.protocol https` (Not tested yet) *

- [Docker](https://docs.docker.com/install/). 
  Additionally, the step to generate static binaries requires you to be part of the `docker` group.
  ```bash
  $ sudo usermod -a -G docker ${USER}
  $ # Reinitialize user env for the user
  $ newgrp -
  ```

- Get the [Packaging](https://github.com/kata-containers/packaging) Kata repository

  ```bash
  $ go get -d github.com/kata-containers/packaging
  ```

## Release process

Notes:

- The steps described here are safe to repeat more than one time, it is safe to
  repeat in case of unexpected issues. And is it not required start from the
  beginning.


- It is "safe" to run this process on any machine. It creates all assets in
  sub-directories and should not modify the entire system.

```bash
$ cd ${GOPATH}/src/github.com/kata-containers/packaging
# make sure you are up-to-date.
$ git pull
```
1. Bump repositories
   ```bash
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
   $ export NEW_VERSION=X.Y.Z
   $ export BRANCH="master"
   $ ./update-repository-version.sh -p "$NEW_VERSION" "$BRANCH"
   ```
   The commands from above will create a GitHub pull request in the Kata projects.
   Work with the Kata approvers to verify that the CI works and the PR are merged.
 
   Note: There is no `VERSION` file in some repositories like `tests`. They are
   tagged with the version that was used to test Kata Containers.
 
2. Create GitHub tags:
   After all the PRs from the previous step are complete, create GitHub tags.
   ```bash
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
   $  ./tag_repos.sh -p -b "$BRANCH" tag
   ```
   This creates tags for all the Kata repos.
 
3. Create the Kata Containers image and upload it to GitHub:
   ```bash
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/obs-packaging
   $ ./gen_versions_txt.sh ${BRANCH}
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
   $ ./publish-kata-image.sh -p ${NEW_VERSION}
   ```
 
4. Create the Kata static binaries tarball and upload it to GitHub::
   ```bash
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
   $ ./kata-deploy-binaries.sh -p ${NEW_VERSION}
   ```
 
5. Create Kata packages:
   ```bash
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/obs-packaging
   # Optional, if release is a new stable branch ./create-repo-branch.sh ${BRANCH}
   $ ./gen_versions_txt.sh ${BRANCH}
   $ PUSH=1 OBS_SUBPROJECT="releases:$(uname -m):${BRANCH}" ./build_from_docker.sh ${NEW_VERSION}
   ```

6. Test packages
After all the packages have built successfully (see status in OBS web page: https://build.opensuse.org/project/subprojects/home:katacontainers),
make sure the packages install and work. To help with this you can use the [package test job](http://jenkins.katacontainers.io/job/package-release-testing)

 
7. Create release notes:
   ```bash
   $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
   # Note: OLD_VERSION is where the script should start to get changes.
   $ ./runtime-release-notes.sh ${OLD_VERSION} ${NEW_VERSION} > notes.md
   # Add the release notes in GitHub runtime.
   $ hub -C "${GOPATH}/src/github.com/kata-containers/runtime" release edit -F notes.md "${NEW_VERSION}"
   ```
 
7. Announce release:

   Publish in [Slack and Kata mailing list][join-us-kata] that new release is ready.

8. Send changes to upstream.
If you found any issue during the release process and you fix it, please send it back.
After your changes are merged, tag Kata packaging with `${NEW_VERSION}` to identify the code used for the release.


[release-process-definition]: https://github.com/kata-containers/documentation/blob/master/Releases.md
[release-checklist]: https://github.com/kata-containers/documentation/blob/master/Release-Checklist.md
[join-us-kata]: https://github.com/kata-containers/community#join-us
[install-go-kata]: https://github.com/kata-containers/documentation/blob/master/Developer-Guide.md#requirements-to-build-individual-components
