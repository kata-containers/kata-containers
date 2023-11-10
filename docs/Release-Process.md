# How to do a Kata Containers Release
  This document lists the tasks required to create a Kata Release.

## Requirements

- [gh](https://cli.github.com)
  * Install and configure the GitHub CLI (gh) as detailed at https://docs.github.com/en/github-cli/github-cli/quickstart#prerequisites .

- GitHub permissions to push tags and create releases in the Kata repository.

- GPG configured to sign git tags. https://docs.github.com/en/authentication/managing-commit-signature-verification/generating-a-new-gpg-key

- `gh auth login` should have configured `git push` and `git pull` to use HTTPS along with your GitHub credentials,
  * As an alternative, you can still rely on SSH keys to push branches. See https://help.github.com/articles/adding-a-new-ssh-key-to-your-github-account .

## Release Process


### Bump the Kata repository

  Bump the repository using the `./update-repository-version.sh` script in the Kata [release](../tools/packaging/release) directory, where:
  - `BRANCH=<the-branch-you-want-to-bump>`
  - `NEW_VERSION=<the-new-kata-version>`
  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
  $ export NEW_VERSION=<the-new-kata-version>
  $ export BRANCH=<the-branch-you-want-to-bump>
  $ ./update-repository-version.sh -p "$NEW_VERSION" "$BRANCH"
  ```

### Merge the bump version Pull request

  - The above step will create a GitHub pull request in the Kata repository. Trigger the CI using `/test` command on the bump Pull request.
  - Check any failures and fix if needed.
  - Work with the Kata approvers to verify that the CI works and the pull request is merged.

### Tag the Kata repository

  Once the pull request to bump version in the Kata repository is merged,
  tag the repository as shown below.
  ```
  $ cd ${GOPATH}/src/github.com/kata-containers/kata-containers/tools/packaging/release
  $ git checkout  <kata-branch-to-release>
  $ git pull
  $ ./tag_repos.sh -p -b "$BRANCH" tag
  ```

### Check Git-hub Actions

  We make use of [GitHub actions](https://github.com/features/actions) in this [file](../.github/workflows/release.yaml) in the `kata-containers/kata-containers` repository to build and upload release artifacts. This action is auto triggered with the above step when a new tag is pushed to the `kata-containers/kata-containers` repository.

  Check the [actions status page](https://github.com/kata-containers/kata-containers/actions) to verify all steps in the actions workflow have completed successfully. On success, a static tarball containing Kata release artifacts will be uploaded to the [Release page](https://github.com/kata-containers/kata-containers/releases).

### Create release notes

  We have the `./release-notes.sh` script in the [release](../tools/packaging/release) directory to create release notes that include a short-log of the commits.

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
