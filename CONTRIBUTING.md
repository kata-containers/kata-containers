# Contribute to the Kata Containers project

* [Code of Conduct](#code-of-conduct)
* [AI generated code](#ai-generated-code)
* [Pull requests](#pull-requests)
* [GitHub basic setup](#github-basic-setup)
    * [Prerequisites](#prerequisites)
    * [Golang coding style](#golang-coding-style)
    * [Rust coding style](#rust-coding-style)
    * [Certificate of Origin](#certificate-of-origin)
* [GitHub best practices](#github-best-practices)
    * [Submit issues before PRs](#submit-issues-before-prs)
    * [Issue tracking](#issue-tracking)
    * [Closing issues](#closing-issues)
* [GitHub workflow](#github-workflow)
    * [Configure your environment](#configure-your-environment)
    * [GitHub labels and keywords that block PRs](#github-labels-and-keywords-that-block-prs)
* [Use static checks for validation](#use-static-checks-for-validation)
    * [Fix failed static checks after submitting PRs](#fix-failed-static-checks-after-submitting-prs)
    * [Kata runtime static checks](#kata-runtime-static-checks)
* [Patch format](#patch-format)
    * [General format](#general-format)
    * [Subsystem](#subsystem)
    * [Best practices for patches](#best-practices-for-patches)
    * [Verification](#verification)
    * [Examples](#examples)
* [Reviews](#reviews)
    * [Review Examples](#review-examples)
* [Continuous Integration](#continuous-integration)
* [Contact](#contact)
* [Project maintainers](#project-maintainers)

The Kata Containers project is an open source project licensed under the
[Apache License, Version 2.0](https://www.apache.org/licenses/LICENSE-2.0).

It comprises a number of repositories under the [GitHub Kata
Containers organisation](https://github.com/kata-containers). Unless
explicitly stated otherwise, all the Kata Containers repositories follow the
process documented here.

[`AGENTS.md`](AGENTS.md) is a companion to this document aimed at AI coding
agents. It describes the layout of this repository, the checks to run locally
before submitting a pull request, and the traps that catch newcomers. Anybody
sending a first patch may find it a useful quick reference.

## Code of Conduct

All contributors must agree to the project [code of conduct](CODE_OF_CONDUCT.md).

## AI generated code

Contributions produced with the help of AI tools are accepted, provided
they follow the
[OpenInfra Foundation AI Policy](https://openinfra.org/legal/ai-policy/).
In particular, contributors must:

* Disclose AI usage in the commit message using the trailers defined by
  the policy:
    * `Assisted-By:` for predictive auto-complete or minor generative
      assistance.
    * `Generated-By:` when substantial portions of the change were
      produced by a generative AI tool.
* Stay "in the loop" and be able to fully understand, explain, and debug
  any AI-produced code they submit.
* Configure AI tools to respect open source licensing, and ensure the
  output is compatible with the project license and does not infringe
  third-party copyrights.

The standard [Certificate of Origin](#certificate-of-origin) sign-off
still applies, and reviewers may apply additional scrutiny to AI-assisted
contributions as the policy recommends.

### Best practices

#### Don't use AI for prose

Writing your own commit messages and PR summaries shows care and thoughtfulness in your contributions.
It's fine if English isn't your first language — perfect grammar isn't required.
More practically, these descriptions help maintainers understand what you've done and why, saving everyone time.

Similarly, avoid opening issues or posting comments that are entirely AI-generated.
Issues are meant for genuine, thoughtful community discussion.

Long blocks of AI-generated prose are unlikely to be prioritized by committers. A little extra effort writing
clearly and concisely will generally get your work reviewed and merged more quickly.

This applies especially to CVE reports: AI can be a useful tool for finding vulnerabilities, but you should
understand the trust model and the implications yourself, and ensure that the finding is a genuine vulnerability
that breaks the trust model and not just a bug (in which case an issue is welcomed),then write your own focused
description rather than submitting whatever the AI produced.

## Pull requests

All the repositories accept contributions via [GitHub Pull requests (PR)](https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/about-pull-requests). Submit PRs by following the [GitHub workflow](#github-workflow).

## GitHub basic setup

To get started, complete the prerequisites below.

### Prerequisites

- [Set up Git](https://help.github.com/en/github/getting-started-with-github/set-up-git).

  >**Note:** The email address you specify must match the email address you
  >use to sign-off commits.

- [Fork and Clone](https://help.github.com/en/github/getting-started-with-github/fork-a-repo) the relevant repository at the
  [Kata Containers Project](https://github.com/kata-containers).

   Example: Your local clone should show `your-github-username`, as follows.
   `https://github.com/${your-github-username}/kata-containers`.

### Golang coding style

* Review [Go Code Review Comments](https://go.dev/wiki/CodeReviewComments) to avoid common `Golang` errors.
* Use `gofmt` to fix any mechanical style issues.

### Rust coding style

* Use `rustfmt` to fix any mechanical style issues. `Rustfmt` uses a style which conforms to the
[Rust Style Guide](https://github.com/rust-dev-tools/fmt-rfcs/blob/master/guide/guide.md).
* Use `clippy` to catch common mistakes and improve your Rust code.

You can install the above tools as follows.

```sh
$ rustup component add rustfmt clippy
```

### Certificate of Origin

In order to get a clear contribution chain of trust we use the [signed-off-by
language](https://ltsi.linuxfoundation.org/software/signed-off-process/)
used by the Linux kernel project.

## GitHub best practices

### Submit issues before PRs

It can be helpful to raise a GitHub issue before starting work on a PR for a new feature/bug to let the community
know that you are planning to work on an item.

### Issue tracking

To report a bug that is not already documented, please open a GitHub issue for the kata-containers repo.

### Closing issues

If you are fixing an issues with a commit, then it's helpful to add a `Fixes` comment to at
least one commit in the PR, which triggers GitHub to automatically close the issue once the
PR is merged:

```
pod: Remove token from Cmd structure

The token and pid data will be hold by the new Process structure and
they are related to a container.

Fixes: #123

Signed-off-by: Sebastien Boeuf <sebastien.boeuf@intel.com>
```

The issue is automatically closed by GitHub when the
[commit message](https://help.github.com/articles/closing-issues-via-commit-messages/) is parsed.

## GitHub workflow

Kata Containers employs certain augmentations to a
[standard GitHub workflow](https://docs.github.com/en/get-started/using-github/github-flow).
In this section, we explain these augmentations in more detail. Follow these guidelines when contributing to Kata Containers repositories, except where noted below.

* Complete the [GitHub basic setup](#github-basic-setup) above before continuing.

* Ensure each PR only covers one topic. If you mix up different items in
  your patches or PR, they will likely need to be reworked.

* Follow a [topic branch method](https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/about-branches) for development.

* Follow carefully the [patch format](#patch-format) for PRs.

* Apply the appropriate GitHub labels to your PR.
  See also [GitHub labels and keywords that block PRs](#github-labels-and-keywords-that-block-prs)

  >**Note:** External contributors should use keywords, explained in the
  >link above. Labels may not be visible to external contributors.

* [Rebase](https://help.github.com/en/github/using-git/about-git-rebase)
  commits on your branch and `force push` after each cycle of feedback.

### Configure your environment

By convention in Go, code is put inside the directory specified by the `$GOPATH`
variable. Follow this example to put the code in the standard location.

```sh
$ export GOPATH=${GOPATH:-$HOME/go}
$ mkdir -p "$GOPATH"
```

For further details on `golang`, refer to the
[requirements section of the Kata Developer Guide](https://github.com/kata-containers/kata-containers/blob/main/docs/Developer-Guide.md#requirements-to-build-individual-components).

>*Note*: If you intend to make minor edits, it's acceptable
> to simply fork and clone without adding the GOPATH variable.

#### Fork and clone

In this example, we configure a Git environment to contribute to this
`kata-containers` repo. We create a sample branch, incorporate reviewer feedback, and rebase our commits.

1. Fork the [upstream repository](https://help.github.com/articles/cloning-a-repository):

1. [Clone your forked copy of the upstream repository](https://help.github.com/articles/cloning-a-repository):

1. While on your *forked copy*, select the green button `Code`
   and copy the URL.

1. Run the commands below and **paste the copied URL** (previous step),
   so your real GitHub user name replaces `your-github-username` below.

```sh
$ dir="$GOPATH/src/github.com/kata-containers"
$ mkdir -p "$dir"
$ cd "$dir"
$ git clone https://github.com/{your-github-username}/kata-containers
$ cd kata-containers
```

>**Note:** Cloning a forked repository automatically gives a remote `origin`.

#### Configure the upstream remote

Next, add the remote `upstream`. Configuring this remote allows you to
synchronize your forked copy, `origin`, with the `upstream`. The
`upstream` URL varies by repository. We use the `upstream` from the kata-containers for this example.

1. Change directory into `kata-containers`.

1. Set the remote `upstream` as follows.

    ```sh
    $ git remote add upstream https://github.com/kata-containers/kata-containers
    ```

1. Run `git remote -v`. Your remotes should appear similar to these:

    ```
    origin  https://github.com/your-github-username/kata-containers.git (fetch)
    origin  https://github.com/your-github-username/kata-containers.git (push)
    upstream  https://github.com/kata-containers/kata-containers (fetch)
    upstream  https://github.com/kata-containers/kata-containers (push)
    ```

For more details, see how to [set up a git remote](https://help.github.com/articles/configuring-a-remote-for-a-fork).

#### Create a topic branch

1. Create a new "topic branch" to do your work on:

    ```
    $ git checkout -b fix-contrib-bugs
    ```

    >**Warning:** *Never* make changes directly to the `main` branch
    >--*always* create a new "topic branch" for PR work.

1. Make some editorial changes. In this example, we modify the file that
   you are reading.

    ```
    $ $EDITOR CONTRIBUTING.md
    ```

   >**Note:** If editing in Windows make sure that all documents end with LF
   > and not CRLF. The CI system will fail if carriage returns are in the
   > document. Many editors support the ability to change this. There is a
   > tool called dos2unix available on Git Bash for Windows and also available
   > on Linux systems that can convert files to LF endings. See the
   > [Configuring Git to handle line endings](https://docs.github.com/en/github/getting-started-with-github/getting-started-with-git/configuring-git-to-handle-line-endings)
   > guide for more details on how to configure `git` to automatically insert
   > the correct line endings.

1. Commit your changes to the current (`fix-contrib-bugs`) branch. Assure
   you use the correct [patch format](#patch-format):

    ```
    $ git commit -as
    ```

1. Push your local `fix-contrib-bugs` branch to your remote fork:

    ```
    $ git push -u origin fix-contrib-bugs
    ```

   >**Note:** The `-u` option tells `git` to "link" your local clone with
   > your remote fork so that it knows from now on that the local repository
   > and the remote fork refer to "the same" upstream repository. Strictly
   >speaking, this option is only required the first time you call `git push`
   >for a new clone.

1. Create the PR:
  - Browse to https://github.com/kata-containers/kata-containers.
  - Click the "Compare & pull request" button that appears.
  - Click the "Create pull request" button.

  >**Note:** You do not need to change any of the defaults on this page.

#### Update your PR based on review comments

Suppose you received some reviewer feedback that asked you to make some
changes to your PR. You updated your local branch and committed those
review changes by creating three commits. There are now four commits in your
local branch: the original commit you created for the PR and three other
commits you created to apply the review changes to your branch. Your branch
now looks something like this:

```sh
$ git log main.. --oneline --decorate=no
4928d57 docs: Fix typos and fold long lines
829c6c8 apply review feedback changes
7c9b1b2 remove blank lines
60e2b2b doh - missed one
```

>**Note:** The `git log` command compares your current branch
>(`fix-contrib-bugs`) with the `main` branch and lists all the commits,
>one per line.

#### Git rebase if multiple commits

Since all four commits are related to *the same change*, it makes sense to
combine all four commits into a *single commit* on your PR. You need to
[git rebase](https://help.github.com/github/using-git/about-git-rebase)
multiple commits on your branch. Follow these steps.

1. Update the `main` branch in your local copy of the upstream
   repository:

    ```
    $ cd $GOPATH/src/github.com/kata-containers/kata-containers
    $ git checkout main
    $ git pull --rebase upstream main
    ```

    The previous command downloads all the latest changes from the upstream
    repository and adds them to your *local copy*.

1. Now, switch back to your PR branch:

    ```
    $ git checkout fix-contrib-bugs
    ```

1. Rebase your changes against the `main` branch.

    ```sh
    $ git rebase -i main
    ```

    Example output:

    ```sh
    pick 2e335ac docs: Fix typos and fold long lines
    pick 6d6deb0 apply review feedback changes
    pick 23bc01d remove blank lines
    pick 3a4ba3f doh - missed one
    ```

1. In your editor, read the comments at the bottom of the screen.
   Do not modify the first line, `pick 2e335ac docs: Fix typos ...`. Instead, revise `pick` to `squash` at the start of all following lines.

    Example output:

    ```sh
    pick 2e335ac docs: Fix typos and fold long lines
    squash 6d6deb0 apply review feedback changes
    squash 23bc01d remove blank lines
    squash 3a4ba3f doh - missed one
    ```

1. Save your changes and quit the editor. Git puts you *back* into your
   editor. You will see all the commit *messages*.

1. At top is your first commit, which should be in the
   [correct format](#patch-format). Keep your first commit and delete
   all the following commits, as appropriate, based on
   the review feedback.

1. Save the file and quit the editor. Once this operation completes, the four
   commits will have been converted into a single new commit. Check this by
   running the `git log` command again:

    ```sh
    $ git log main.. --oneline --decorate=no
    3ea3aef docs: Fix typo
    ```

1. Force push your updated local `fix-contrib-bugs` branch to `origin`
   remote:

    ```sh
    $ git push -f origin fix-contrib-bugs
    ```

>**Note:** Not only does this command upload your changes to your fork, it
>also includes the *latest upstream changes* to your fork since you ran
>`git pull --rebase upstream main` on the main branch and then merged
>those changes into your PR branch. This ensures your fork is now "up to
>date" with the upstream repository. The `-f` option is a "force push". Since
>you created a new commit using `git rebase`, you must "overwrite" the old
>copy of your branch in your fork on GitHub.

Your PR is now updated on GitHub. To ensure team members are aware of this,
leave a message on the PR stating something like, "Review feedback applied".
This notification allows the team to once again review your PR more quickly.

### GitHub labels and keywords that block PRs

Kata Containers CI systems have two methods that allow marking
PRs to prevent them being merged. This practice is often used during
development. The two methods are: 1) Use
[GitHub labels](https://help.github.com/articles/about-labels/)
or; 2) Use keywords in the PR subject line. The keywords can appear anywhere
in the subject line.

The following table summarises some common scenarios and appropriate use
of labels or keywords:

| Scenario | GitHub label | PR description contains |
| -------- | ------------ | ----------------------- |
| PR created "as an idea" and feedback sought | `rfc` | RFC |
| PR incomplete - needs more work or rework | `do-not-merge` `wip` | WIP |
| PR should not be merged (has all required approvals, but needs more reviewer input) | `do-not-merge` | |
| PR is a "work In progress", raised to get early feedback | `wip` | WIP |
| PR is complete but depends on another so should not be merged (yet) | `do-not-merge` | |

If any of the values in the table above are set on a PR, it will be
automatically blocked from merging.

>**Note:** Often during discussions, the abbreviated and full terms are
>used interchangeably. For instance, often `DNM` is used in discussions as
>shorthand for `do-not-merge`. The CI systems only recognise the above
>phrases as shown.

## Use static checks for validation

* Kata Containers utilizes [Continuous Integration (CI)](#continuous-integration) to automatically check every PR.

* We strongly encourage you to run the same CI tests on individual PRs, using [static checks](https://github.com/kata-containers/tests/blob/main/.ci/static-checks.sh)

In repositories where a `Makefile` is present, you can execute
static checks for testing and development. To do so, invoke the `make check` and `make test` rules, after developer mode is enabled.

```sh
$ export KATA_DEV_MODE=true
$ make check
$ make test
```
Running these checks should result in **no errors**. If errors are reported, fix them before submitting your PR.

To replicate the static checks performed by the CI system:

- Ensure you have a "clean" source tree, as the checks cover all files
  present. Checks might fail if you have extra files or your files are out of date in your tree.

- Ensure [`golangci-lint`](https://github.com/golangci/golangci-lint) is
current or has not been previously installed (the static check scripts will
install it if necessary). Changing the linters used or the Kata
Containers code base can produce spurious errors that do not fail inside the
CI systems.

### Fix failed static checks after submitting PRs

Some submitted PRs fail to pass static checks. After such a PR fails,
view its build logs to determine the cause of failure.

### Kata runtime static checks

If working on `kata-runtime`, first ensure you run `make` and `make install`
in the `virtcontainers` subdirectory, as shown below. For more information,
see [virtcontainers](https://github.com/kata-containers/kata-containers/blob/main/src/runtime/virtcontainers/documentation/Developers.md#testing).

```bash
$ pushd runtime/virtcontainers
$ make
$ sudo -E PATH=$PATH make install
$ popd
```

>**Note:** The final `popd` is required to return to the top-level directory
>from where other build rules can be executed.

## Patch format

### General format

Beside the `Signed-off-by` footer, we expect each patch to comply with the
following format:

```
subsystem: One line change summary

More detailed explanation of your changes (why and how)
that spans as many lines as required.

A "Fixes: #XXX" comment listing the GitHub issue this change resolves.
This comment is required for the main patch in a sequence. See the following examples.

Signed-off-by: Contributors Name <contributor@foo.com>
```

#### Pull request format

As shown above, pull requests must adhere to these guidelines:

* Preface the PR title with the appropriate keyword found in [Subsystem](#subsystem)

* Ensure PR title length is 75 characters or fewer, including whichever
  `subsystem` term is used.

* Ensure the PR body line length is 150 characters or fewer.

The body of the message is **not** a continuation of the subject line and is
not used to extend the subject line beyond its character limit. The subject
line is a complete sentence and the body is a complete, standalone paragraph.

Additionally, PR body line length is not checked for special lines:

* Lines that do not start with an alphabetic character

* Sign-off lines (as to not penalize long names)

* Lines without spaces

These should allow contributors to pass checks while including relevant
information **if needed**.

### Subsystem

The "subsystem" describes the area of the code that the change applies to.
It does not have to match a particular directory name in the source tree
because it is a "hint" to the reader. The subsystem is generally a single
word. Although the subsystem must be specified, it is not validated. The
author decides what is a relevant subsystem for each patch.

Examples:

| Subsystem | Description |
|--|--|
| `build` | `Makefile` or configuration script change |
| `cli` | Change affecting command line options or commands |
| `docs` | Documentation change |
| `logging` | Logging change |

To see the subsystem values chosen for existing commits:

```
$ git log --no-merges --pretty="%s" | cut -d: -f1 | sort -u
```

### Best practices for patches

We recommend that each patch fixes one thing. Smaller patches are easier to
review, more likely to be accepted and merged, and more conducive for
identifying problems during review.

A PR can contain multiple patches. These patches should generally be related
to the [main patch](#main-patch) and the overall goal of the PR. However, it
is also acceptable to include additional or
[supplementary patches](#supplementary-patch) for things such as:

- Formatting (or whitespace) fixes
- Comment improvements
- Tidy up work
- Refactoring to simplify the codebase

### Verification

Correct formatting of the PR patches is verified using the commit-message-check GitHub Action.

### Examples

#### Main patch

The following is an example of a full patch description for the main change that shows the required "`Fixes: #XXX`" comment, which references the GitHub issue this patch resolves:

```
pod: Remove token from Cmd structure

The token and pid data will be hold by the new Process structure and
they are related to a container.

Fixes: #123

Signed-off-by: Sebastien Boeuf <sebastien.boeuf@intel.com>
```

#### Supplementary patch

If a PR contains multiple patches, [only one of those patches](#main-patch) needs to specify the "`Fixes: #XXX`" comment. Supplementary patches have an identical format to the main patch, but do not need to specify a "`Fixes: #XXX`"
comment.

Example:

```
image-builder: Fix incorrect error message

Fixed an error message which was referring to an incorrect rootfs
variable name.

Signed-off-by: James O. D. Hunt <james.o.hunt@intel.com>
```

## Reviews

Before your PRs are merged into the main code base, they are reviewed. We
encourage anybody to review any PR and leave feedback.

See the [PR review guide](https://github.com/kata-containers/community/blob/main/PR-Review-Guide.md) for tips on performing a
careful review.

We use the GitHub [Required Reviews](https://help.github.com/articles/approving-a-pull-request-with-required-reviews/)
system for reviewers to note if they agree or disagree with a PR. To have
an acknowledgment or "nack" registered with GitHub, you **must** use the
GitHub "Review changes" dialog to leave feedback. Notes left only in the
comments fields, whilst sometimes useful, will not get registered
in the acknowledgment counting system.

Documentation PRs can sometimes use a modified process explained in the
[Documentation Review Process](https://github.com/kata-containers/community/blob/main/Documentation-Review-Process.md) guide.

### Review Examples

The following is an example of a valid "ack", as long as
the "Approve" box is ticked in the Review changes dialog:

```
Excellent work - thanks for your contribution.

lgtm
```

## Continuous Integration

The Kata Containers project has a gating process to prevent introducing
regressions. When your PR is submitted, a Continuous Integration (CI) system
will run different checks on different platforms, based upon your changes. Currently Kata uses GitHub actions
for testing your changes.

Some of the checks are:

- Static analysis checks.
- Unit tests.
- Functional tests.
- Integration tests.

The CI tests jobs will wait to be triggered. A maintainer must add an `ok-to-test`
label on the PR to let the CI jobs run.

All CI jobs must pass in order to merge your PR.

## Contact

The Kata Containers community can be reached
[through various channels](README.md#join-us).

## Project maintainers

The Kata Containers project maintainers are the people accepting or
rejecting any PR. Although [anyone can review PRs](#reviews), only the
acknowledgement (or "ack") from an Approver counts towards the approval of a PR.

Approvers are listed in GitHub teams. The project
uses the
[GitHub required status checks](https://help.github.com/en/articles/enabling-required-status-checks)
along with the [GitHub `CODEOWNERS`file](https://help.github.com/en/articles/about-code-owners) to specify who can approve PRs. All repositories are configured to require:

- Two approvals from the repository-specific approval team.

- One [documentation team](https://github.com/orgs/kata-containers/teams/documentation/members) approval if the PR modifies documentation.
