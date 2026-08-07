# Contribute to the Kata Containers project

* [Code of Conduct](#code-of-conduct)
* [AI generated code](#ai-generated-code)
* [Pull requests](#pull-requests)
* [GitHub basic setup](#github-basic-setup)
    * [Prerequisites](#prerequisites)
    * [Contributor roles](#contributor-roles)
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
* [Re-vendor PRs](#re-vendor-prs)
* [Use static checks for validation](#use-static-checks-for-validation)
    * [Fix failed static checks after submitting PRs](#fix-failed-static-checks-after-submitting-prs)
    * [Kata runtime static checks](#kata-runtime-static-checks)
* [Porting](#porting)
    * [Porting labels](#porting-labels)
    * [Stable branch backports](#stable-branch-backports)
    * [Porting issue numbers](#porting-issue-numbers)
    * [Porting comments](#porting-comments)
    * [Forward ports](#forward-ports)
    * [Porting examples](#porting-examples)
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

> **Note:**
>
> This document originates in the
> [community repository](https://github.com/kata-containers/community/blob/main/CONTRIBUTING.md)
> and lives here so that it is available to contributors and to tooling that
> does not follow links out of the repository. A few statements that had gone
> out of date were corrected on the way in. Process changes should land in both
> places.

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

## Pull requests

All the repositories accept contributions via [GitHub Pull requests (PR)](https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/about-pull-requests). Submit PRs by following the [GitHub workflow](#github-workflow).

## GitHub basic setup

To get started, complete the prerequisites below.

### Prerequisites

- Review [Contributor roles](#contributor-roles) that require special Git
  configuration.

- [Set up Git](https://help.github.com/en/github/getting-started-with-github/set-up-git).

  >**Note:** The email address you specify must match the email address you
  >use to sign-off commits.

- [Fork and Clone](https://help.github.com/en/github/getting-started-with-github/fork-a-repo) the relevant repository at the
  [Kata Containers Project](https://github.com/kata-containers).

   Example: Your local clone should show `your-github-username`, as follows.
   `https://github.com/${your-github-username}/community`.

### Contributor roles

Special Git configuration is required for these contributors:

* [Golang coding style](#golang-coding-style)
* [Rust coding style](#rust-coding-style)
* [Kata runtime static checks](#kata-runtime-static-checks)

For all other contributor roles, follow the standard configuration, shown in
Prerequisites.

### Golang coding style

* Review [Go Code Review Comments](https://github.com/golang/go/wiki/CodeReviewComments) to avoid common `Golang` errors.
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

To report a bug that is not already documented, please open a GitHub issue for the repository in question.

If it is unclear which repository to raise your query against, first try to
get in [contact](#contact) with us. If in doubt, raise the issue
[here](https://github.com/kata-containers/community/issues/new) and we will
help you to handle the query by routing it to the correct area for resolution.

### Closing issues

When a PR resolves a reported issue, add a `Fixes` comment to at least one of its commits. This is not enforced by tooling, but it triggers GitHub to close the issue automatically once the PR is merged:

```
pod: Remove token from Cmd structure

The token and pid data will be hold by the new Process structure and
they are related to a container.

Fixes #123

Signed-off-by: Sebastien Boeuf <sebastien.boeuf@intel.com>
```

The issue is automatically closed by GitHub when the
[commit message](https://help.github.com/articles/closing-issues-via-commit-messages/) is parsed.

## GitHub workflow

Kata Containers employs certain augmentations to a
[standard GitHub workflow](https://guides.github.com/introduction/flow/).
In this section, we explain these augmentations in more detail. Follow these guidelines when contributing to Kata Containers repositories, except where noted below.

* Complete the [GitHub basic setup](#github-basic-setup) above before continuing.

* Ensure each PR only covers one topic. If you mix up different items in
  your patches or PR, they will likely need to be reworked.

* Follow a [topic branch method](https://help.github.com/en/github/collaborating-with-issues-and-pull-requests/about-branches) for development.

* Follow carefully the [patch format](#patch-format) for PRs.

* Apply the appropriate GitHub labels to your PR. This
  is particularly relevant to maintain [Stable branch backports](#stable-branch-backports). See also [GitHub labels and keywords that block PRs](#github-labels-and-keywords-that-block-prs)

  >**Note:** External contributors should use keywords, explained in the
  >link above. Labels may not be visible to external contributors.

* [Rebase](https://help.github.com/en/github/using-git/about-git-rebase)
  commits on your branch and `force push` after each cycle of feedback.

### Configure your environment

Most [Kata Containers repositories](https://github.com/kata-containers)
contain code written in the [Go language (golang)](https://golang.org/). Go
requires all code to be put inside the directory specified by the `$GOPATH`
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

In this example, we configure a Git environment to contribute to this very
`Community` repo. We create a sample branch, incorporate reviewer feedback, and rebase our commits.

1. Fork the [upstream repository](https://help.github.com/articles/cloning-a-repository):

1. [Clone your forked copy of the upstream repository](https://help.github.com/articles/cloning-a-repository):

1. While on your *forked copy*, select the green button `Clone or download`
   and copy the URL.

1. Run the commands below and **paste the copied URL** (previous step),
   so your real GitHub user name replaces `your-github-username` below.

```sh
$ dir="$GOPATH/src/github.com/kata-containers"
$ mkdir -p "$dir"
$ cd "$dir"
$ git clone https://github.com/{your-github-username}/community
$ cd community
```

>**Note:** Cloning a forked repository automatically gives a remote `origin`.

#### Configure the upstream remote

Next, add the remote `upstream`. Configuring this remote allows you to
synchronize your forked copy, `origin`, with the `upstream`. The
`upstream` URL varies by repository. We use the `upstream` from the Community for this example.

1. Change directory into `community`.

1. Set the remote `upstream` as follows.

    ```sh
    $ git remote add upstream https://github.com/kata-containers/community
    ```

1. Run `git remote -v`. Your remotes should appear similar to these:

    ```
    origin  https://github.com/your-github-username/community.git (fetch)
    origin  https://github.com/your-github-username/community.git (push)
    upstream  https://github.com/kata-containers/community (fetch)
    upstream  https://github.com/kata-containers/community (push)
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
  - Browse to https://github.com/kata-containers/community.
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
    $ cd $GOPATH/src/github.com/kata-containers/community
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

## Re-vendor PRs

If you raise a PR to update the vendored copy of one or more golang packages,
after running the
[`dep`](https://github.com/kata-containers/community/blob/main/VENDORING.md) command, ensure you add any modified files under the `vendor/` directory to Git before committing the changes:

```sh
$ git add vendor/
```

There are two critical pieces of information you need to add to the commit
body:

- A brief explanation why the re-vendor is required.

  For example, you should state if an important bug fix or new feature is
  required, or if a particular commit is needed.

- The range of commits being added for these third-party packages.

  It is possible that re-vendoring a particular package will also result in
  updates to other dependent packages. However, it is important to include
  the commit range (even if it is big) for the primary package(s) the
  re-vendor PR is raised for.

  These details allow for easier troubleshooting if the re-vendor PR
  introduces bug or behavioral changes.

  Generate the list of new commits added to the primary re-vendored
  package by comparing the previous and latest commits for the package being re-vendored.

  The following example lists the steps you should take if a new version of
  `libcontainer` (part of the `runc` repository) is required:

  1. Determine the previous and latest commits for the package by looking at
     the `diff` of the `Gopkg.toml` file in your branch.

  1. Run the commands below:

     ```bash
     $ go get -d -u github.com/opencontainers/runc
     $ cd $GOPATH/src/github.com/opencontainers/runc
     $ old_commit="..."
     $ new_commit="..."
     $ git log --no-merges --abbrev-commit --pretty=oneline "${old_commit}..${new_commit}" | sed 's/^/    /g'
     ```

  Paste the output of the previous command directly into the commit "as-is".
  Note that the four space indent added by the `sed` command is used to force
  GitHub to render the list in a fixed-width font, which makes it easier to
  read.

For additional information on using the `dep` tool, see
"[Performing vendoring for the Kata Containers project](https://github.com/kata-containers/community/blob/main/VENDORING.md)".

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

- [x] Ensure you have a "clean" source tree, as the checks cover all files
  present. Checks might fail if you have extra files or your files are out of date in your tree.

- [x] Ensure [`golangci-lint`](https://github.com/golangci/golangci-lint) is
current or has not been previously installed (the static check scripts will
install it if necessary). Changing the linters used or the Kata
Containers code base can produce spurious errors that do not fail inside the
CI systems.

### Fix failed static checks after submitting PRs

Some submitted PRs fail to pass static checks. After such a PR fails,
view its build logs to determine the cause of failure.

1. At the bottom of the PR, if a message appears, "Some checks were not
   successful,"  select "Details", as shown below.

    ![Failed CI-CD](https://raw.githubusercontent.com/kata-containers/community/main/fig1-ci-cd-failure.png)

1. Upon entering the Travis CI* web page, select the first number that
   appears below "Build jobs."

1. Scroll to the bottom of the build log and view the `ERROR` message(s).
   In the example below, the `ERROR` reads: `... no
   signed-off-by specified`. This is a requirement. To fix, use the signed-off-by method while pushing a commit. See [Patch format](
   #patch-format) for more details.

    ![Build log error messages](https://raw.githubusercontent.com/kata-containers/community/main/fig2-ci-cd-log.png)

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

## Porting

Porting applies a patch set to an older ("backport") or a newer
("forward-port") branch or repository.

Backporting is necessary to ensure that older -- but still maintained --
releases benefit from bug fixes already applied to newer releases.

Forward porting is necessary where there are multiple development streams and
bug fixes or new features have been applied to the older stream, but not the
newer one.

> **Note:**
>
> Stable branches are considered maintenance branches, not development
> branches. Bug fixes must land in a newer development branch before landing
> in a stable branch to ensure the changes have been tested thoroughly before
> being applied to a stable release (maintenance) branch.

Porting is performed with a new PR meaning porting PRs *must* have an
associated "parent" PR (the original bug fix or feature PR).

Every PR must indicate whether it should be ported in either direction;
*backwards* (backport) or *forwards* (forward port). This is achieved by
adding up to two labels per PR which signal the porting requirements for the
PR.

The [stable branch backports](#stable-branch-backports) section provides
information on the sorts of changes which should be backported.

### Porting labels

The table below lists all valid combinations of GitHub labels. Every PR must
be labelled as shown in the table row that most closely corresponds to the
type of PR the user is raising.

> **Notes:**
>
> - The porting labels are enforced by a
>   [GitHub action](https://github.com/kata-containers/.github/blob/main/scripts/pr-porting-checks.sh).
>   This means that *PRs that do not have a valid set of porting labels cannot be merged*.
> - The "Common PR type" column in the table shows the most likely type of PR, but
>   this is just a guide.
> - A `backport` or `forward-port` labelled PR **must** have an associated
>   parent PR which caused the backport or forward port PR to be raised.

| PR summary | Common PR type | Backport label | Forward port label | Notes |
|-|-|-|-|-|
| A "standalone" PR | Feature | `no-backport-needed` | `no-forward-port-needed` | PR does not need to be ported. For example, a PR used to add a new feature to the latest release. |
| PR that needs to be backported only | Bug fix | `needs-backport` | `no-forward-port-needed` | |
| PR that needs to be forward ported only | Bug fix or feature | `no-backport-needed` | `needs-forward-port` | |
| PR that needs to be ported backwards *and* forwards | Bug fix | `needs-backport` |  `needs-forward-port` | For example a Kata 1.x PR that needs to be ported to Kata 2.0 *and* to one or more Kata 1.x stable branches. |
| A backport PR | Bug fix | `backport` | | PR to actually make the backport changes.<br/><br/>Must have an associated "parent" PR.<br/><br/>Title **must** contain original PRs title. |
| A forward port PR | Bug fix or feature | |  `forward-port` | PR to actually make the forward port changes.<br/><br/>Must have an associated "parent" PR.<br/><br/>Title **must** contain original PRs title. |

If you are not a member of the GitHub repository the PR is raised in, you may
not be able to see the GitHub labels. In this scenario, please add a comment
asking for the porting labels to be applied.

Forward port and backport PRs by definition should not be raised in isolation:
there must be an existing PR that caused the porting PR to be raised.

If you know whether a PR should be backported or forward ported, please add
a comment on the PR if you are unable to add the appropriate labels. If you do
not know whether a PR should be backported or forward ported, the community
will work with you to identify any porting requirements and to help with
porting activities.

### Stable branch backports

Kata Containers maintains a number of stable branch releases. Bug fixes to
the main branch are selectively applied to (or "backported") these stable branches.

In order to aid identification of commits that potentially should be
backported to the stable branches, all PRs submitted must be labeled with
one or more of the following labels. At least one label that is *not*
`stable-candidate` must be included.

| Label              | Meaning |
| -----              | ------- |
| `bug`              | A bug fix, which will potentially be a backport candidate |
| `cleanup`          | A cleanup, which will likely not be backported                 |
| `feature`          | A new feature/enhancement, that will likely not be backported  |
| `stable-candidate` | A PR selected for backporting - very likely a bug fix          |
| `vendor`           | A golang vendor update. Might be considered for backport if the vendor update includes critical bug fixes |

In the event that a bug fix PR is selected for backporting to the stable
branches, the `stable-candidate` label is added if not already present, and
the original author of the PR is asked if they will submit the relevant
backport PRs. For a quick guide on how to perform and submit a backport, see
the [Backport Guide](https://github.com/kata-containers/community/blob/main/Backport-Guide.md) in this repository.

### Porting issue numbers

For ports that are within the same repository (for example a stable backport
to a 1.x PR), specify the same issue number as the original PR in the "fixes
comment". See the [patch format](#patch-format) section for further details.

For ports in different repositories, create a new issue, referencing the
original issue URL in the issue text.

### Porting comments

Issues and PRs can only be linked if they are within the same repository.
Since Kata 2.x uses a new central repository, it is essential to add a special
comment to the **original PR** when new port PRs are created.

| Port type | Port comment format |
|-|-|
| backport | `backport PR: <backport-pr-url>` |
| forward port| `forward port PR: <forward-port-pr-url>` |

> **Notes:**
>
> - The special comments **must** appear at the start of a line.
>
> - The special comments are used by tooling to check that porting has been
>   completed correctly.
>
> - Although these comments strictly make the `backport` and `forward-port`
>   labels redundant, these labels are useful for general reporting since
>   users can search for "all port PRs in a particular repository" for
>   example.

### Forward ports

Forward ports tend to be much less common than backports. However,
consolidating a number of standalone repositories into a single repository for
the Kata 2.0 development effort introduced a potential forward port
requirement. At the time, both Kata 1.x and 2.0 versions were being developed
in parallel. This meant that lots of PRs raised in the Kata 1.x repositories
needed to be forward ported to the Kata 2.0 repository (since this was to be
the next major release and needed to contain all bug fixes and features where
possible).

| Initial PR raised | PR type | Backport? | Forward port? |
|-|-|-|-|
| Particular 1.x repository | bug fix | stable branches | 2.0 repository |
| Particular 1.x repository | feature | | 2.0 repository |
| The 2.0 repository | bug fix | 1.x (and maybe stable branches) | - |
| The 2.0 repository | feature | | - |

### Porting examples

#### Backport and forward port example

Imagine that you have just raised a new PR on a Kata 1.x repository. The PR
contains three commits:

- A commit to fix a "typo" in a comment.
- A commit that changes the way a container is destroyed (and updates the tests).
- A commit that updates the documentation explaining the new contain
  destruction behaviour.

Since the PR is not adding any new functionality and since it is correcting
problems with existing code, it can be considered a bug fix PR. Bug fixes
should generally be backported. However, this PR was raised in a Kata 1.x
repository meaning it should also potentially be forward-ported to the Kata
2.0 repository.

Looking at the tables in the [porting labels](#porting-labels) and the [stable
branch backports](#stable-branch-backports) sections shows that this PR needs
to be labelled with the following labels:

- `needs-backport`
- `needs-forward-port`
- `stable-candidate`

Once this PR is approved and is merged in the Kata 1.x repository, it should
now be backported and forward ported:

- Backport: Raise new PRs on the currently maintained stable releases

  - These PRs should be labelled with the `backport` label.
  - The commit message should mention the original PR.
  - The commit message should reference the *original* issue number in the
    ["fixes" comment](#patch-format).

- Forward port: Raise a new PR on the Kata 2.0 repository

  - This PR should be labelled with the `forward-port` label.
  - The commit message should mention the original PR.
  - A new issue should be used for the PR and the original issue URL
    referenced in the issue text.

- Add [porting comments](#porting-comments) to the *original PR* with two comments,
  one for each port:

  ```
  backport PR: https://github.com/kata-containers/runtime/pull/XXX
  forward port PR: https://github.com/kata-containers/kata-containers/pull/YYY
  ```

#### Double backport example

Imagine that you have just raised a new PR on the Kata 2.x repository. The PR
fixes a nasty runtime bug, and adds some new unit tests to guarantee no future
regression.

This is a pure bug fix PR. There is no "newer" branch or version, so it is not
possible to forward port. However, since the PR is an important bug fix, it
should be backported, both to Kata 1.x *and* the Kata 1.x stable branches.

Looking at the tables in the [porting labels](#porting-labels) and the [stable
branch backports](#stable-branch-backports) sections shows that this PR needs
to be labelled with the following labels:

- `needs-backport`
- `no-forward-port-needed`
- `stable-candidate`

Once this PR is approved and is merged in the Kata 2.0 repository, it should
now be backported *twice*:

- Kata 1.x: Raise a new PR on the Kata 1.x runtime repository

  - This PR should be labelled with the `backport` label.
  - The commit message should mention the original PR.

- Stable backports: Raise new PRs on each of the currently maintained stable
  branches in the Kata 1.x runtime repository

  - These PRs should be labelled with the `backport` label.
  - The commit message should mention the original PR.
  - The commit message should reference the issue number used in the
    ["fixes" comment](#patch-format) for the 1.x PR
    (since that is in the same repository).

- Add [porting comments](#porting-comments) to the *original PR* with two comments,
  one for each port:

  ```
  backport PR: https://github.com/kata-containers/runtime/pull/XXX
  backport PR: https://github.com/kata-containers/runtime/pull/YYY
  ```

## Patch format

### General format

Beside the `Signed-off-by` footer, we expect each patch to comply with the
following format:

```
subsystem: One line change summary

More detailed explanation of your changes (why and how)
that spans as many lines as required.

A "Fixes #XXX" comment listing the GitHub issue this change resolves, if it
resolves one. Add it to the main patch in a sequence. See the following examples.

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
| `vendor` | [Re-vendoring](#re-vendor-prs) change |

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

The following is an example of a full patch description for the main change, showing a "`Fixes #XXX`" comment that references the GitHub issue this patch resolves:

```
pod: Remove token from Cmd structure

The token and pid data will be hold by the new Process structure and
they are related to a container.

Fixes: #123

Signed-off-by: Sebastien Boeuf <sebastien.boeuf@intel.com>
```

#### Supplementary patch

If a PR contains multiple patches, [only one of those patches](#main-patch) should specify the "`Fixes #XXX`" comment. Supplementary patches have an identical format to the main patch, but do not specify a "`Fixes #XXX`"
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
will run different checks on different platforms, based upon your changes. Currently Kata uses [Jenkins](http://jenkins.katacontainers.io)
for testing your changes.

Some of the checks are:

- Static analysis checks.
- Unit tests.
- Functional tests.
- Integration tests.

The Jenkins jobs will wait to be triggered. A maintainer must add a `/test`
comment on the PR to let the CI jobs run.

All CI jobs must pass in order to merge your PR.

## Contact

The Kata Containers community can be reached
[through various channels](https://github.com/kata-containers/community#join-us).

## Project maintainers

The Kata Containers project maintainers are the people accepting or
rejecting any PR. Although [anyone can review PRs](#reviews), only the
acknowledgement (or "ack") from an Approver counts towards the approval of a PR.

Approvers are listed in GitHub teams, one for each repository. The project
uses the
[GitHub required status checks](https://help.github.com/en/articles/enabling-required-status-checks)
along with the [GitHub `CODEOWNERS`file](https://help.github.com/en/articles/about-code-owners) to specify who can approve PRs. All repositories are configured to require:

- Two approvals from the repository-specific approval team.

- One [documentation team](https://github.com/orgs/kata-containers/teams/documentation/members) approval if the PR modifies documentation.
