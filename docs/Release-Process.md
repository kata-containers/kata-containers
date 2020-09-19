
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
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
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
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
  $ git checkout  <kata-branch-to-release>
  $ git pull
  $ ./tag_repos.sh -p -b "$BRANCH" tag
  ```

### Check Git-hub Actions

  We make use of [GitHub actions](https://github.com/features/actions) in this [file](https://github.com/kata-containers/kata-containers/blob/master/.github/workflows/main.yaml) in the `kata-containers/kata-containers` repository to build and upload release artifacts. This action is auto triggered with the above step when a new tag is pushed to the `kata-containers/kata-conatiners` repository.

  Check the [actions status page](https://github.com/kata-containers/kata-containers/actions) to verify all steps in the actions workflow have completed successfully. On success, a static tarball containing Kata release artifacts will be uploaded to the [Release page](https://github.com/kata-containers/kata-containers/releases).

### Create release notes

  We have a script in place in the packaging repository to create release notes that include a short-log of the commits across Kata components.

  Run the script as shown below:

  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
  # Note: OLD_VERSION is where the script should start to get changes.
  $ ./runtime-release-notes.sh ${OLD_VERSION} ${NEW_VERSION} > notes.md
  # Edit the `notes.md` file to review and make any changes to the release notes.
  # Add the release notes in GitHub runtime.
  $ hub release edit -F notes.md "${NEW_VERSION}"
  ```

### Announce the release

  Publish in [Slack and Kata mailing list](https://github.com/kata-containers/community#join-us) that new release is ready.
