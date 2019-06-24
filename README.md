# Kata Containers tests

* [Kata Containers tests](#kata-containers-tests)
    * [Getting the code](#getting-the-code)
    * [Test Content](#test-content)
    * [CI Content](#ci-content)
        * [Centralised scripts](#centralised-scripts)
        * [CI setup](#ci-setup)
        * [Detecting a CI system](#detecting-a-ci-system)
        * [Breaking Compatibility](#breaking-compatibility)
    * [CLI tools](#cli-tools)
    * [Developer Mode](#developer-mode)
    * [Write a new Unit Test](#write-a-new-unit-test)
    * [Run the Kata Containers tests](#run-the-kata-containers-tests)
        * [Requirements to run Kata Containers tests](#requirements-to-run-kata-containers-tests)
        * [Prepare an environment](#prepare-an-environment)
        * [Run the tests](#run-the-tests)
        * [Running subsets of tests](#running-subsets-of-tests)
            * [Running `Bats` based tests](#running-bats-based-tests)
            * [Running `Ginkgo` based tests](#running-ginkgo-based-tests)
    * [Metrics tests](#metrics-tests)
    * [Kata Admission controller webhook](#kata-admission-controller-webhook)

This repository contains various types of tests and utilities (called
"content" from now on) for testing the [Kata Containers](https://github.com/kata-containers)
code repositories.

## Getting the code

```
$ go get -d github.com/kata-containers/tests
```

## Test Content

We provide several tests to ensure Kata-Containers run on different scenarios
and with different container managers.

1. [Functional tests](https://github.com/kata-containers/tests/tree/master/functional)
2. Integration tests to ensure compatibility with:
   - [Docker](https://github.com/kata-containers/tests/tree/master/integration/docker)
   - [Kubernetes](https://github.com/kata-containers/tests/tree/master/integration/kubernetes)
   - [CRI-O](https://github.com/kata-containers/tests/tree/master/integration/cri-o)
   - [Containerd](https://github.com/kata-containers/tests/tree/master/integration/containerd)
   - [OpenShift](https://github.com/kata-containers/tests/tree/master/integration/openshift)
3. [Network tests](https://github.com/kata-containers/tests/tree/master/integration/network)
4. [Stability tests](https://github.com/kata-containers/tests/tree/master/integration/stability)
5. [Metrics](https://github.com/kata-containers/tests/tree/master/metrics)

> **Note:** The unit tests of the different Kata Containers components are stored in each repository
> along with the source code they test.

## CI Content

This repository contains a [number of scripts](/.ci)
that run from under a "CI" (Continuous Integration) system.

### Centralised scripts

The CI scripts in this repository are used to test changes to the content of
this repository. These scripts are also used by the other Kata Containers code
repositories.

The advantages of this approach are:

- Functionality is defined once.
  - Easy to make changes affecting all code repositories centrally.

- Assurance that all the code repositories are tested in this same way.

CI scripts also provide a convenient way for other Kata repositories to
install software. The preferred way to use these scripts is to invoke `make`
with the corresponding `install-` target. For example, to install CRI-O you
would use:

```
$ make -C <path-to-this-repo> install-crio
```

Use `make list-install-targets` to retrieve all the available install targets.

### CI setup

> **WARNING:**
>
> The CI scripts perform a lot of setup before running content under a
> CI. Some of this setup runs as the `root` user and **could break your developer's
> system**. See [Developer Mode](#developer-mode).

### Detecting a CI system

The strategy to check if the tests are running under a CI system is to see
if the `CI` variable is set to the value `true`. For example, in shell syntax:

```
if [ "$CI" = true ]; then
    : # Assumed to be running in a CI environment
else
    : # Assumed to NOT be running in a CI environment
fi
```

### Breaking Compatibility

In case the patch you submit breaks the CI because it needs to be tested
together with a patch from another `kata-containers` repository, you have to
specify which repository and which pull request it depends on.

Using a simple tag `Depends-on:` in your commit message will allow the CI to
run properly. Notice that this tag is parsed from the latest commit of the
pull request.

For example:

```
	Subsystem: Change summary

	Detailed explanation of your changes.

	Fixes: #nnn

	Depends-on:github.com/kata-containers/runtime#999

	Signed-off-by: <contributor@foo.com>

```

In this example, we tell the CI to fetch the pull request 999 from the `runtime`
repository and use that rather than the `master` branch when testing the changes
contained in this pull request.

## CLI tools

This repository contains a number of [command line tools](cmd). They are used
by the [CI](#ci-content) tests but may be useful for user to run stand alone.

## Developer Mode

Developers need a way to run as much test content as possible locally, but as
explained in [CI Setup](#ci-setup), running *all* the content in this
repository could be dangerous.

The recommended approach to resolve this issue is to set the following variable
to any non-blank value **before using *any* content from this repository**:

```
export KATA_DEV_MODE=true
```

Setting this variable has the following effects:

- Disables content that might not be safe for developers to run locally.
- Ignores the effect of the `CI` variable being set (for extra safety).

You should be aware that setting this variable provides a safe *subset* of
functionality; it is still possible that PRs raised for code repositories will
still fail under the automated CI systems since those systems are running all
possible tests.

## Write a new Unit Test

See the [unit test advice documentation](Unit-Test-Advice.md).

## Run the Kata Containers tests

### Requirements to run Kata Containers tests

You need to install the following to run Kata Containers tests:

- [golang](https://golang.org/dl)

  To view the versions of go known to work, see the `golang` entry in the
  [versions database](https://github.com/kata-containers/runtime/blob/master/versions.yaml).

- `make`.

### Prepare an environment

The recommended method to set up Kata Containers is to use the official and latest
stable release. You can find the official documentation to do this in the
[Kata Containers installation user guides](https://github.com/kata-containers/documentation/blob/master/install/README.md).

To try the latest commits of Kata use the CI scripts, which build and install from the
`kata-containers` repositories, with the following steps:

> **Warning:** This may replace/delete packages and configuration that you already have.
> Please use these steps only on a testing environment.

Add the `$GOPATH/bin` directory to the PATH:
```
$ export PATH=${GOPATH}/bin:${PATH}
```

Clone the `kata-container/tests` repository:
```
$ go get -d github.com/kata-containers/tests
```

Go to the tests repo directory:
```
$ cd $GOPATH/src/github.com/kata-containers/tests
```

Execute the setup script:
```
$ .ci/setup.sh
```
> **Limitation:** If the script fails for a reason and it is re-executed, it will execute
all steps from the beginning and not from the failed step.

### Run the tests

If you have already installed the Kata Containers packages and a container
manager (i.e. Docker or Kubernetes), and you want to execute the content
for all the tests, run the following:

```
$ export RUNTIME=kata-runtime
$ export KATA_DEV_MODE=true
$ sudo -E PATH=$PATH make test
```

You can also execute a single test suite. For example, if you want to execute
the docker integration tests, run the following:
```
$ sudo -E PATH=$PATH make docker
```

A list of available test suite `make` targets can be found by running the
following:

```
$ make help
```

### Running subsets of tests

Individual tests or subsets of tests can be selected to be run. The method of
test selection depends on which type of test framework the test is written
with. Most of the Kata Containers test suites are written
using either [Bats](https://github.com/sstephenson/bats) files or with
[Ginkgo](https://github.com/onsi/ginkgo).

#### Running Bats based tests

The Bats based tests are shell scripts, starting with the line:

```sh
#!/usr/bin/env bats
```

This allows the Bats files to be executed directly.  Before executing the file,
ensure you have Bats installed. The Bats files should be executed
from the root directory of the tests repository to ensure they can locate all other
necessary components. An example of how a Bats test is run from the `Makefile`
looks like:

```makefile
kubernetes:
        bash -f .ci/install_bats.sh
        bash -f integration/kubernetes/run_kubernetes_tests.sh
```

#### Running Ginkgo based tests

Ginkgo supports selecting, filtering and excluding tests to be run using command
line parameters. The tests repository `Makefile` supports passing the environment
variables `FOCUS` and `SKIP` through to the `ginkgo` command in most cases. Check
the `Makefile` specifics before you use this functionality.

Ginkgo accepts [Golang regexp](https://golang.org/pkg/regexp/) regular expressions
for the `FOCUS` and `SKIP` arguments. See the
[Ginkgo documentation](https://onsi.github.io/ginkgo/#focused-specs) for more
specifics. Example:

```sh
$ make docker					# all tests
$ export FOCUS=".*CPU.*"; $ make docker		# any test with 'CPU' in its name
$ export FOCUS="CPU constraints"; $ make docker	# One specific test
$ export SKIP="CPU constraints"; $ make docker	# Skip one specific test
```

## Metrics tests

See the [metrics documentation](metrics).

## Kata Admission controller webhook

See the [webhook documentation](kata-webhook).
