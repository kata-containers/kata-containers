# How to do a Kata Containers Release
  This document lists the tasks required to create a Kata Release.

## Requirements

- [gh](https://cli.github.com)
  * Install and configure the GitHub CLI (gh) as detailed at https://docs.github.com/en/github-cli/github-cli/quickstart#prerequisites .

- GitHub permissions to push tags and create releases in Kata repositories.

- GPG configured to sign git tags. https://docs.github.com/en/authentication/managing-commit-signature-verification/generating-a-new-gpg-key

- `gh auth login` should have configured `git push` and `git pull` to use HTTPS along with your GitHub credentials,
  * As an alternative, you can still rely on SSH keys to push branches. See https://help.github.com/articles/adding-a-new-ssh-key-to-your-github-account .

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

### Point tests repository to stable branch

  If your release changes a major or minor version number(not a patch release), then the above 
  `./tag_repos.sh` script will create a new stable branch in all the repositories in addition to tagging them.
  This happens when you are making the first `rc` release for a new major or minor version in Kata.
  In this case, you should modify the `tests` repository to point to the newly created stable branch and not the `main` branch.
  The objective is that changes in the CI on the main branch will not impact the stable branch.

  In the test directory, change references of the `main` branch to the new stable branch in:
  * `README.md`
  * `versions.yaml`
  * `cmd/github-labels/labels.yaml.in`
  * `cmd/pmemctl/pmemctl.sh`
  * `.ci/lib.sh`
  * `.ci/static-checks.sh`

  See the commits in [the corresponding PR for stable-2.1](https://github.com/kata-containers/tests/pull/3504) for an example of the changes.

### Check Git-hub Actions

  We make use of [GitHub actions](https://github.com/features/actions) in this [file](../.github/workflows/release.yaml) in the `kata-containers/kata-containers` repository to build and upload release artifacts. This action is auto triggered with the above step when a new tag is pushed to the `kata-containers/kata-containers` repository.

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
  $ gh release edit "${NEW_VERSION}" -F notes.md
  ```

### Announce the release

  Publish in [Slack and Kata mailing list](https://github.com/kata-containers/community#join-us) that new release is ready.
