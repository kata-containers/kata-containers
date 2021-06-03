
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
  * Using an [application token](https://github.com/settings/tokens) is required for hub.

- GitHub permissions to push tags and create releases in Kata repositories.

- GPG configured to sign git tags. https://help.github.com/articles/generating-a-new-gpg-key/

- You should configure your GitHub to use your ssh keys (to push to branches). See https://help.github.com/articles/adding-a-new-ssh-key-to-your-github-account/.
    * As an alternative, configure hub to push and fork with HTTPS, `git config --global hub.protocol https` (Not tested yet) *

## Release Process


### Bump all Kata repositories

  Bump the repositories using a script in the Kata packaging repo, where:
  - `BRANCH=<the-branch-you-want-to-bump>`
  - `NEW_VERSION=<the-new-kata-version>`
  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
  $ export NEW_VERSION=<the-new-kata-version>
  $ export BRANCH=<the-branch-you-want-to-bump>
  $ ./update-repository-version.sh -p "$NEW_VERSION" "$BRANCH"
  ```

### Point tests repository to stable branch

  If you create a new stable branch, i.e. if your release changes a major or minor version number (not a patch release), then
  you should modify the `tests` repository to point to that newly created stable branch and not the `main` branch.
  The objective is that changes in the CI on the main branch will not impact the stable branch.

  In the test directory, change references the main branch in:
  * `README.md`
  * `versions.yaml`
  * `cmd/github-labels/labels.yaml.in`
  * `cmd/pmemctl/pmemctl.sh`
  * `.ci/lib.sh`
  * `.ci/static-checks.sh`

  See the commits in [the corresponding PR for stable-2.1](https://github.com/kata-containers/tests/pull/3504) for an example of the changes.


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

  We make use of [GitHub actions](https://github.com/features/actions) in this [file](https://github.com/kata-containers/kata-containers/blob/main/.github/workflows/main.yaml) in the `kata-containers/kata-containers` repository to build and upload release artifacts. This action is auto triggered with the above step when a new tag is pushed to the `kata-containers/kata-containers` repository.

  Check the [actions status page](https://github.com/kata-containers/kata-containers/actions) to verify all steps in the actions workflow have completed successfully. On success, a static tarball containing Kata release artifacts will be uploaded to the [Release page](https://github.com/kata-containers/kata-containers/releases).

### Create release notes

  We have a script in place in the packaging repository to create release notes that include a short-log of the commits across Kata components.

  Run the script as shown below:

  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
  # Note: OLD_VERSION is where the script should start to get changes.
  $ ./release-notes.sh ${OLD_VERSION} ${NEW_VERSION} > notes.md
  # Edit the `notes.md` file to review and make any changes to the release notes.
  # Add the release notes in the project's GitHub.
  $ hub release edit -F notes.md "${NEW_VERSION}"
  ```

### Announce the release

  Publish in [Slack and Kata mailing list](https://github.com/kata-containers/community#join-us) that new release is ready.
