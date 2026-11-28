# How to do a Kata Containers Release

This document lists the tasks required to create a Kata Release.

## Requirements

- GitHub permissions to run workflows.

## Release Model

Kata Containers follows a rolling release model with monthly snapshots.
New features, bug fixes, and improvements are continuously integrated into
`main`. Each month, a snapshot is tagged as a new `MINOR` release. The
target release date is no earlier than the first Tuesday on or after
the 15th of each month. The actual release date may be later, but any
changes merged before the target date are expected to make it into the release.

### Versioning

Releases use the `MAJOR.MINOR.PATCH` scheme. Monthly snapshots increment
`MINOR`; `PATCH` is typically `0`. Major releases are rare (years apart) and
signal significant architectural changes that may require updates to container
managers (Containerd, CRI-O) or other infrastructure. Breaking changes in
`MINOR` releases are avoided where possible, but may occasionally occur as
features are deprecated or removed.

### No Stable Branches

The Kata Containers project does not maintain stable branches (see
[#9064](https://github.com/kata-containers/kata-containers/issues/9064)).
Bug fixes land on `main` and ship in the next monthly snapshot rather than
being backported. Downstream projects that need extended support or compliance
certifications should select a monthly snapshot as their stable base and manage
their own validation and patch backporting from there.

## Release Process

### Lock the `main` branch and announce release process

In order to prevent any PRs getting merged during the release process, and
slowing the release process down, by impacting the payload caches, we have
recently trialed setting the `main` branch to read-only.
Once the `kata-containers/kata-containers` repository is ready for a new
release, lock the main branch until the release action has completed.
Notify the #kata-dev Slack channel about the ongoing release process.
Ideally, CI usage by others should be reduced to a minimum during the
ongoing release process.

> [!NOTE]
> Admin permission is needed to lock/unlock the `main` branch.

### Draft the release notes and request reviews

Once the `main` branch is locked, trigger the [Draft Kata release
notes](https://github.com/kata-containers/kata-containers/actions/workflows/draft-release-notes.yaml)
workflow from `main`, with the following inputs:

- `version`: the version being released, for example `4.4.0`.
- `previous_tag`: the tag of the previous release.
- `target`: optionally, the full SHA of the commit being released. It defaults
  to the tip of `main` at the time of dispatch.
- `certify_dco`: must be ticked, to authorize the DCO sign-off on the generated
  commit.

The workflow drafts `docs/releases/<version>.md` from the changes between the
previous tag and the target, validates it (spelling, EditorConfig and a website
build), and opens a draft PR containing the release notes, the
[`docs/.nav.yml`](./.nav.yml) entry and any new dictionary words.

Request reviews on the draft PR from the people who worked on the release, and
poke @channel on #kata-dev so that they can check the notes are accurate and
highlight the most important features and bug fixes. The draft is generated
automatically, so reviewers should check its claims, and resolve the HTML
comments that flag gaps in the evidence, before the PR is marked ready.

> [!NOTE]
> This PR independent of the VERSION bump, so both can be created at the same time and
can be merged in any order, however the VERSION PR has a longer follow-on, so priority
should be given to it.

### Bump the `VERSION` and `Chart.yaml` file

Create a PR, separate from the release notes PR, to set the release in the
[`VERSION`](./../VERSION) file and to update the `version` and `appVersion`
fields in the
[`Chart.yaml`](./../tools/packaging/kata-deploy/helm-chart/kata-deploy/Chart.yaml)
file. Temporarily unlock the main branch to merge the PR.

### Wait for the `VERSION` bump PR payload publish to complete

To reduce the chance of need to re-run the release workflow, check the [CI |
Publish Kata Containers
payload](https://github.com/kata-containers/kata-containers/actions/workflows/payload-after-push.yaml)
once the `VERSION` PR bump has merged to check that the assets build correctly
and are cached, so that the release process can just download these artifacts
rather than needing to build them all, which takes time and can reveal errors in
infra.

### Merge the release notes PR

Once the reviews on the release notes PR have been addressed, mark the PR as
ready and merge it, temporarily unlocking the `main` branch if required. Merge
the notes before triggering the release below.

### Trigger the `Release Kata Containers` GitHub Action

We make use of [GitHub actions](https://github.com/features/actions) in the
[release](https://github.com/kata-containers/kata-containers/actions/workflows/release.yaml)
file from the `kata-containers/kata-containers` repository to build and upload
release artifacts.

> [!NOTE]
> Write permissions to trigger the action.

The action is manually triggered and is responsible for generating a new release
(including a new tag), pushing those to the `kata-containers/kata-containers`
repository. The new release is initially created as a draft. It is promoted to
an official release when the whole workflow has completed successfully.

Check the [actions status
page](https://github.com/kata-containers/kata-containers/actions) to verify all
steps in the actions workflow have completed successfully. On success, a static
tarball containing Kata release artifacts will be uploaded to the [Release
page](https://github.com/kata-containers/kata-containers/releases).

If the workflow fails because of some external environmental causes, e.g.
network timeout, simply re-run the failed jobs until they eventually succeed.

If for some reason you need to cancel the workflow or re-run it entirely, go
first to the [Release
page](https://github.com/kata-containers/kata-containers/releases) and delete
the draft release from the previous run.

### Unlock the `main` branch

After the release process has concluded, either unlock the `main` branch, or ask
an admin to do it.

### Announce the release

Publish in [Slack and Kata mailing
list](https://github.com/kata-containers/community#join-us) that new release is
ready.
