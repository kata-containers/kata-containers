# Agent Control tool

* [Overview](#overview)
* [Audience and environment](#audience-and-environment)
* [Full details](#full-details)

## Overview

The Kata Containers agent control tool (`kata-agent-ctl`) is a low-level test
tool. It allows basic interaction with the Kata Containers agent,
`kata-agent`, that runs inside the virtual machine.

Unlike the Kata Runtime, which only ever makes sequences of correctly ordered
and valid agent API calls, this tool allows users to make arbitrary agent API
calls and to control their parameters.

## Audience and environment

> **Warning:**
>
> This tool is for *advanced* users familiar with the low-level agent API calls.
> Further, it is designed to be run on test and development systems **only**: since
> the tool can make arbitrary API calls, it is possible to easily confuse
> irrevocably other parts of the system or even kill a running container or
> sandbox.

## Full details

For a usage statement, run:

```sh
$ cargo run -- --help
```

To see some examples, run:

```sh
$ cargo run -- examples
```
