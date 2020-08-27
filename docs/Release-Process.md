
# How to do a Kata Containers Release
  This document lists the tasks required to create a Kata Release.

<!-- TOC START min:1 max:3 link:true asterisk:false update:true -->
- [How to do a Kata Containers Release](#how-to-do-a-kata-containers-release)
  - [Requirements](#requirements)
  - [Release Process](#release-process)
    - [Bump all Kata repositories](#bump-all-kata-repositories)
    - [Merge all bump version Pull requests](#merge-all-bump-version-pull-requests)
    - [Tag all Kata repositories](#tag-all-kata-repositories)
    - [Check Git-hub Actions](#check-git-hub-actions)
    - [Create OBS Packages](#create-obs-packages)
    - [Create release notes](#create-release-notes)
    - [Announce the release](#announce-the-release)
<!-- TOC END -->


## Requirements

- [hub](https://github.com/github/hub)

- OBS account with permissions on [`/home:katacontainers`](https://build.opensuse.org/project/subprojects/home:katacontainers)

- GitHub permissions to push tags and create releases in Kata repositories.

- GPG configured to sign git tags. https://help.github.com/articles/generating-a-new-gpg-key/

- You should configure your GitHub to use your ssh keys (to push to branches). See https://help.github.com/articles/adding-a-new-ssh-key-to-your-github-account/.
    * As an alternative, configure hub to push and fork with HTTPS, `git config --global hub.protocol https` (Not tested yet) *

## Release Process

### Bump all Kata repositories

  - We have set up a Jenkins job to bump the version in the `VERSION` file in all Kata repositories. Go to the [Jenkins bump-job page](http://jenkins.katacontainers.io/job/release/build) to trigger a new job.
  - Start a new job with variables for the job passed as:
     - `BRANCH=<the-branch-you-want-to-bump>`
     - `NEW_VERSION=<the-new-kata-version>`

     For example, in the case where you want to make a patch release `1.10.2`, the variable `NEW_VERSION` should be `1.10.2` and `BRANCH` should point to  `stable-1.10`. In case of an alpha or release candidate release, `BRANCH` should point to `master` branch.

  Alternatively, you can also bump the repositories using a script in the Kata packaging repo
  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
  $ export NEW_VERSION=<the-new-kata-version>
  $ export BRANCH=<the-branch-you-want-to-bump>
  $ ./update-repository-version.sh -p "$NEW_VERSION" "$BRANCH"
  ```

### Merge all bump version Pull requests

  - The above step will create a GitHub pull request in the Kata projects. Trigger the CI using `/test` command on each bump Pull request.
  - Check any failures and fix if needed.
  - Work with the Kata approvers to verify that the CI works and the pull requests are merged.

### Tag all Kata repositories

  Once all the pull requests to bump versions in all Kata repositories are merged,
  tag all the repositories as shown below.  
  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
  $ git checkout  <kata-branch-to-release>
  $ git pull
  $ ./tag_repos.sh -p -b "$BRANCH" tag
  ```

### Check Git-hub Actions

  We make use of [GitHub actions](https://github.com/features/actions) in this [file](https://github.com/kata-containers/kata-containers/blob/master/.github/workflows/main.yaml) in the `kata-containers/kata-containers` repository to build and upload release artifacts. This action is auto triggered with the above step when a new tag is pushed to the `kata-containers/kata-conatiners` repository.

  Check the [actions status page](https://github.com/kata-containers/kata-containers/actions) to verify all steps in the actions workflow have completed successfully. On success, a static tarball containing Kata release artifacts will be uploaded to the [Release page](https://github.com/kata-containers/kata-containers/releases).

### Create OBS Packages

  - We have set up an [Azure Pipelines](https://azure.microsoft.com/en-us/services/devops/pipelines/) job
  to trigger generation of Kata packages in [OBS](https://build.opensuse.org/).
  Go to the [Azure Pipelines job that creates OBS packages](https://dev.azure.com/kata-containers/release-process/_release?_a=releases&view=mine&definitionId=1).
  - Click on "Create release" (blue button, at top right corner).
    It should prompt you for variables to be passed to the release job. They should look like:

    ```
    BRANCH="the-kata-branch-that-is-release"
    BUILD_HEAD=false
    OBS_BRANCH="the-kata-branch-that-is-release"
    ```
    Note: If the release is `Alpha` , `Beta` , or `RC` (that is part of a `master` release), please use `OBS_BRANCH=master`.

    The above step shall create OBS packages for Kata for various distributions that Kata supports and test them as well.
  - Verify that the packages have built successfully by checking the [Kata OBS  project page](https://build.opensuse.org/project/subprojects/home:katacontainers).
  - Make sure packages work correctly. This can be done manually or via the [package testing pipeline](http://jenkins.katacontainers.io/job/package-release-testing).
    You have to make sure the packages are already published by OBS before this step.
    It should prompt you for variables to be passed to the pipeline:

    ```
    BRANCH="<kata-branch-to-release>"
    NEW_VERSION=<the-version-you-expect-to-be-packaged|latest>
    ```
    Note: `latest` will verify that a package provides the latest Kata tag in that branch.

### Create release notes

  We have a script in place in the packaging repository to create release notes that include a short-log of the commits across Kata components.

  Run the script as shown below:

  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/packaging/release
  # Note: OLD_VERSION is where the script should start to get changes.
  $ ./runtime-release-notes.sh ${OLD_VERSION} ${NEW_VERSION} > notes.md
  # Edit the `notes.md` file to review and make any changes to the release notes.
  # Add the release notes in GitHub runtime.
  $ hub -C "${GOPATH}/src/github.com/kata-containers/runtime" release edit -F notes.md "${NEW_VERSION}"
  ```

### Announce the release

  Publish in [Slack and Kata mailing list](https://github.com/kata-containers/community#join-us) that new release is ready.
