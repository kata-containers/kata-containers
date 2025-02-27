# Kata Containers CI

> [!WARNING]
> While this project's CI has several areas for improvement, it is constantly
> evolving. This document attempts to describe its current state, but due to
> ongoing changes, you may notice some outdated information here. Feel free to
> modify/improve this document as you use the CI and notice anything odd. The
> community appreciates it!

## Introduction

The Kata Containers CI relies on [GitHub Actions][gh-actions], where the actions
themselves can be found in the `.github/workflows` directory, and they may call
helper scripts, which are located under the `tests` directory, to actually
perform the tasks required for each test case.

## The different workflows

There are a few different sets of workflows that are running as part of our CI,
and here we're going to cover the ones that are less likely to get rotten.  With
this said, it's fair to advise that if the reader finds something that got
rotten, opening an issue to the project pointing to the problem is a nice way to
help, and providing a fix for the issue is a very encouraging way to help.

### Jobs that run automatically when a PR is raised

These are a bunch of tests that will automatically run as soon as a PR is
opened, they're mostly running on "cost free" runners, and they do some
pre-checks to evaluate that your PR may be okay to start getting reviewed.

Mind, though, that the community expects the contributors to, at least, build
their code before submitting a PR, which the community sees as a very fair
request.

Without getting into the weeds with details on this, those jobs are the ones
responsible for ensuring that:

- The commit message is in the expected format
- There's no missing Developer's Certificate of Origin
- Static checks are passing

### Jobs that require a maintainer's approval to run

There are some tests, and our so-called "CI".  These require a
maintainer's approval to run as parts of those jobs will be running on "paid
runners", which are currently using Azure infrastructure.

Once a maintainer of the project gives "the green light" (currently by adding an
`ok-to-test` label to the PR, soon to be changed to commenting "/test" as part
of a PR review), the following tests will be executed:

- Build all the components (runs on free cost runners, or bare-metal depending on the architecture)
- Create a tarball with all the components (runs on free cost runners, or bare-metal depending on the architecture)
- Create a kata-deploy payload with the tarball generated in the previous step (runs on free costs runner, or bare-metal depending on the architecture)
- Run the following tests:
  - Tests depending on the generated tarball
    - Metrics (runs on bare-metal)
    - `docker` (runs on cost free runners)
    - `nerdctl` (runs on cost free runners)
    - `kata-monitor` (runs on cost free runners)
    - `cri-containerd` (runs on cost free runners)
    - `nydus` (runs on cost free runners)
    - `vfio` (runs on cost free runners)
  - Tests depending on the generated kata-deploy payload
    - kata-deploy (runs on cost free runners)
      - Tests are performed using different "Kubernetes flavors", such as k0s, k3s, rke2, and Azure Kubernetes Service (AKS).
    - Kubernetes (runs in Azure small and medium instances depending on what's required by each test, and on TEE bare-metal machines)
      - Tests are performed with different runtime engines, such as CRI-O and containerd.
      - Tests are performed with different snapshotters for containerd, namely OverlayFS and devmapper.
      - Tests are performed with all the supported hypervisors, which are Cloud Hypervisor, Dragonball, Firecracker, and QEMU.

For all the tests relying on Azure instances, real money is being spent, so the
community asks for the maintainers to be mindful about those, and avoid abusing
them to merely debug issues.

## The different runners

In the previous section we've mentioned using different runners, now in this section we'll go through each type of runner used.

- Cost free runners:  Those are the runners provided by GitHub itself, and
  those are fairly small machines with virtualization capabilities enabled.
- Azure small instances: Those are runners which have virtualization
  capabilities enabled, 2 CPUs, and 8GB of RAM.  These runners have a "-smaller"
  suffix to their name.
- Azure normal instances: Those are runners which have virtualization
  capabilities enabled, 4 CPUs, and 16GB of RAM.  These runners are usually
  `garm` ones with no "-smaller" suffix.
- Bare-metal runners: Those are runners provided by community contributors,
  and they may vary in architecture, size and virtualization capabilities.
  Builder runners don't actually require any virtualization capabilities, while
  runners which will be actually performing the tests must have virtualization
  capabilities and a reasonable amount for CPU and RAM available (at least
  matching the Azure normal instances).

## Adding new tests

Before someone decides to add a new test, we strongly recommend them to go
through [GitHub Actions Documentation][gh-actions],
which will provide you a very sensible background on how to read and understand
current tests we have, and also become familiar with how to write a new test.

On the Kata Containers land, there are basically two sets of tests: "standalone"
and "part of something bigger".

The "standalone" tests, for example the commit message check, won't be covered
here as they're better covered by the GitHub Actions documentation pasted above.

The "part of something bigger" is the more complicated one and not so
straightforward to add, so we'll be focusing our efforts on describing the
addition of those.

> [!NOTE]
> TODO: Currently, this document refers to "tests" when it actually means the
> jobs (or workflows) of GitHub. In an ideal world, except in some specific cases,
> new tests should be added without the need to add new workflows. In the
> not-too-distant future (hopefully), we will improve the workflows to support
> this.

### Adding a new test that's "part of something bigger"

The first important thing here is to align expectations, and we must say that
the community strongly prefers receiving tests that already come with:

- Instructions how to run them
- A proven run where it's passing

There are several ways to achieve those two requirements, and an example of that
can be seen in PR #8115.

With the expectations aligned, adding a test consists in:

- Adding a new yaml file for your test, and ensure it's called from the
  "bigger" yaml. See the [Kata Monitor test example][monitor-ex01].

- Adding the helper scripts needed for your test to run. Again, use the [Kata Monitor script as example][monitor-ex02].

Following those examples, the community advice during the review, and even
asking the community directly on Slack are the best ways to get your test
accepted.

## Required tests

In our CI we have two categories of jobs - required and non-required:
- Required jobs need to all pass for a PR to be merged normally and
should cover all the core features on Kata Containers that we want to
ensure don't have regressions.
- The non-required jobs are for unstable tests, or for features that
are experimental and not-fully supported. We'd like those tests to also
pass on all PRs ideally, but don't block merging if they don't as it's
not necessarily an indication of the PR code causing regressions.

### Transitioning between required and non-required status

Required jobs that fail block merging of PRs, so we want to ensure that
jobs are stable and maintained before we make them required.

The [Kata Containers CI Dashboard](https://kata-containers.github.io/)
is a useful resource to check when collecting evidence of job stability.
At time of writing it reports the last ten days of Kata CI nightly test
results for each job. This isn't perfect as it doesn't currently capture
results on PRs, but is a good guideline for stability.

> [!NOTE]
> Below are general guidelines about jobs being marked as
> required/non-required, but they are subject to change and the Kata
> Architecture Committee may overrule these guidelines at their
> discretion.

#### Initial marking as required

For new jobs, or jobs that haven't been marked as required recently,
the criteria to be initially marked as required is ten days
of passing tests, with no relevant PR failures reported in that time.
Required jobs also need one or more nominated maintainers that are
responsible for the stability of their jobs.

> [!NOTE]
> We don't currently have a good place to record the job maintainers, but
> once we have this, the intention is to show it on the CI Dashboard so
> people can find the contact easily.

#### Expectation of required job maintainers

Due to the nature of the Kata Containers community having contributors
spread around the world, required jobs being blocked due to infrastructure,
or test issues can have a big impact on work. As such, the expectation is
that when a problem with a required job is noticed/reported, the maintainers
have one working day to acknowledge the issue, perform an initial
investigation and then either fix it, or get it marked as non-required
whilst the investigation and/or fix it done.

### Re-marking of required status

Once a job has been removed from the required list, it requires two
consecutive successful nightly test runs before being made required
again.

## Running tests

### Running the tests as part of the CI

If you're a maintainer of the project, you'll be able to kick in the tests by
yourself.  With the current approach, you just need to add the `ok-to-test`
label and the tests will automatically start.  We're moving, though, to use a
`/test` command as part of a GitHub review comment, which will simplify this
process.

If you're not a maintainer, please, send a message on Slack or wait till one of
the maintainers reviews your PR.  Maintainers will then kick in the tests on
your behalf.

In case a test fails and there's the suspicion it happens due to flakiness in
the test itself, please, create an issue for us, and then re-run (or asks
maintainers to re-run) the tests following these steps:

- Locate which tests is failing
- Click in "details"
- In the top right corner, click in "Re-run jobs"
- And then in "Re-run failed jobs"
- And finally click in the green "Re-run jobs" button

> [!NOTE]
> TODO: We need figures here

### Running the tests locally

In this section, aligning expectations is also something very important, as one
will not be able to run the tests exactly in the same way the tests are running
in the CI, as one most likely won't have access to an Azure subscription.
However, we're trying our best here to provide you with instructions on how to
run the tests in an environment that's "close enough" and will help you to debug
issues you find with the current tests, or even provide a proof-of-concept to
the new test you're trying to add.

The basic steps, which we will cover in details down below are:

 1. Create a VM matching the configuration of the target runner
 2. Generate the artifacts you'll need for the test, or download them from a
    current failed run
 3. Follow the steps provided in the action itself to run the tests.

Although the general overview looks easy, we know that some tricks need to be
shared, and we'll go through the general process of debugging one non-Kubernetes
and one Kubernetes specific test for educational purposes.

One important thing to note is that "Create a VM" can be done in innumerable
different ways, using the tools of your choice.  For the sake of simplicity on
this guide, we'll be using `kcli`, which we strongly recommend in case you're a
non-experienced user, and happen to be developing on a Linux box.

For both non-Kubernetes and Kubernetes cases, we'll be using PR #8070 as an
example, which at the time this document is being written serves us very well
the purpose, as you can see that we have `nerdctl` and Kubernetes tests failing.

## Debugging tests

### Debugging a non Kubernetes test

As shown above, the `nerdctl` test is failing.

As a developer you can go ahead to the details of the job, and expand the job
that's failing in order to gather more information.

But when that doesn't help, we need to set up our own environment to debug
what's going on.

Taking a look at the `nerdctl` test, which is located here, you can easily see
that it runs-on a `garm-ubuntu-2304-smaller` virtual machine.

The important parts to understand are `ubuntu-2304`, which is the OS where the
test is running on; and "smaller", which means we're running it on a machine
with 2 CPUs and 8GB of RAM.

With this information, we can go ahead and create a similar VM locally using `kcli`.

```bash
$ sudo kcli create vm -i ubuntu2304 -P disks=[60] -P numcpus=2 -P memory=8192 -P cpumodel=host-passthrough debug-nerdctl-pr8070
```

In order to run the tests, you'll need the "kata-tarball" artifacts, which you
can build your own using "make kata-tarball" (see below), or simply get them
from the PR where the tests failed.  To download them, click on the "Summary"
button that's on the top left corner, and then scroll down till you see the
artifacts, as shown below.

Unfortunately GitHub doesn't give us a link that we can download those from
inside the VM, but we can download them on our local box, and then `scp` the
tarball to the newly created VM that will be used for debugging purposes.

> [!NOTE]
> Those artifacts are only available (for 15 days) when all jobs are finished.

Once you have the `kata-static.tar.xz` in your VM, you can login to the VM with
`kcli ssh debug-nerdctl-pr8070`, go ahead and then clone your development branch

```bash
$ git clone --branch feat_add-fc-runtime-rs https://github.com/nubificus/kata-containers
```

Add the upstream as a remote, set up your git, and rebase your branch atop of the upstream main one

```bash
$ git remote add upstream https://github.com/kata-containers/kata-containers
$ git remote update
$ git config --global user.email "you@example.com"
$ git config --global user.name "Your Name"
$ git rebase upstream/main
```

Now copy the `kata-static.tar.xz` into your `kata-containers/kata-artifacts` directory

```bash
$ mkdir kata-artifacts
$ cp ../kata-static.tar.xz kata-artifacts/
```

> [!NOTE]
> If you downloaded the .zip from GitHub you need to uncompress first to see `kata-static.tar.xz`

And finally run the tests following what's in the yaml file for the test you're
debugging.

In our case, the `run-nerdctl-tests-on-garm.yaml`.

When looking at the file you'll notice that some environment variables are set,
such as `KATA_HYPERVISOR`, and should be aware that, for this particular example,
the important steps to follow are:

Install the dependencies
Install kata
Run the tests

Let's now run the steps mentioned above exporting the expected environment variables

```bash
$ export KATA_HYPERVISOR=dragonball
$ bash ./tests/integration/nerdctl/gha-run.sh install-dependencies
$ bash ./tests/integration/nerdctl/gha-run.sh install-kata
$ bash tests/integration/nerdctl/gha-run.sh run
```

And with this you should've been able to reproduce exactly the same issue found
in the CI, and from now on you can build your own code, use your own binaries,
and have fun debugging and hacking!

### Debugging a Kubernetes test

Steps for debugging the Kubernetes tests are very similar to the ones for
debugging non-Kubernetes tests, with the caveat that what you'll need, this
time, is not the `kata-static.tar.xz` tarball, but rather a payload to be used
with kata-deploy.

In order to generate your own kata-deploy image you can generate your own
`kata-static.tar.xz` and then take advantage of the following script.  Be aware
that the image generated and uploaded must be accessible by the VM where you'll
be performing your tests.

In case you want to take advantage of the payload that was already generated
when you faced the CI failure, which is considerably easier, take a look at the
failed job, then click in "Deploy Kata" and expand the "Final kata-deploy.yaml
that is used in the test" section.  From there you can see exactly what you'll
have to use when deploying kata-deploy in your local cluster.

> [!NOTE]
> TODO: WAINER TO FINISH THIS PART BASED ON HIS PR TO RUN A LOCAL CI

## Adding new runners

Any admin of the project is able to add or remove GitHub runners, and those are
the folks you should rely on.

If you need a new runner added, please, tag @ac in the Kata Containers slack,
and someone from that group will be able to help you.

If you're part of that group and you're looking for information on how to help
someone, this is simple, and must be done in private. Basically what you have to
do is:

- Go to the kata-containers/kata-containers repo
- Click on the Settings button, located in the top right corner
- On the left panel, under "Code and automation", click on "Actions"
- Click on "Runners"

If you want to add a new self-hosted runner:

- In the top right corner there's a green button called "New self-hosted runner"

If you want to remove a current self-hosted runner:

- For each runner there's a "..." menu, where you can just click and the
  "Remove runner" option will show up

## Known limitations

As the GitHub actions are structured right now we cannot: Test the addition of a
GitHub action that's not triggered by a pull_request event as part of the PR.

[gh-actions]: https://docs.github.com/en/actions
[monitor-ex01]: https://github.com/kata-containers/kata-containers/commit/a3fb067f1bccde0cbd3fd4d5de12dfb3d8c28b60
[monitor-ex02]: https://github.com/kata-containers/kata-containers/commit/489caf1ad0fae27cfd00ba3c9ed40e3d512fa492
