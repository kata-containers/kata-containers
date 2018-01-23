# Kata Containers tests

* [Getting the code](#getting-the-code)
* [CI Content](#ci-content)
    * [Centralised scripts](#centralised-scripts)
    * [CI setup](#ci-setup)
    * [Detecting a CI system](#detecting-a-ci-system)
* [Developer Mode](#developer-mode)

This repository contains various types of tests and utilities (called
"content" from now on) for testing the [Kata Containers](https://github.com/kata-containers)
code repositories.

## Getting the code

```
$ go get -d github.com/kata-containers/tests
```

## CI Content

This repository contains a [number of scripts](https://github.com/kata-containers/tests/tree/master/.ci)
that run from under a "CI" (Continuous Integration) system.

### Centralised scripts

The CI scripts in this repository are used to test changes to the content of
this repository. These scripts are also used by the other Kata Containers code
repositories.

The advantages of this approach are:

- Functionality is defined once.
  - Easy to make changes affecting all code repositories centrally.

- Assurance that all the code repositories are tested in this same way.

### CI setup

WARNING:

The CI scripts perform a lot of setup before running content under a
CI. Some of this setup runs as the `root` user and **could break a developer's
system**. See [Developer Mode](#developer-mode).

### Detecting a CI system

The strategy to check if the tests are running under a CI system is to see
if the `CI` variable is set to the value `true`. For example, in shell syntax:

```bash
if [ "$CI" = true ]; then
    # Assumed to be running in a CI environment
else
    # Assumed to NOT be running in a CI environment
fi
```

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
