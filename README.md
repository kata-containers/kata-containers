[![Build Status](https://travis-ci.org/kata-containers/shim.svg?branch=master)](https://travis-ci.org/kata-containers/shim)
[![codecov](https://codecov.io/gh/kata-containers/shim/branch/master/graph/badge.svg)](https://codecov.io/gh/kata-containers/shim)

# Shim

* [Debug mode](#debug-mode)
* [Enable trace support](#enable-trace-support)

This project implements a shim called `kata-shim` for the [Kata
Containers](https://katacontainers.io/) project.

The shim runs in the host environment, handling standard I/O and signals on
behalf of the container process which runs inside the virtual machine.

## Debug mode

To enable agent debug output to the system journal, run with `-debug`.

## Enable trace support

To generate [OpenTracing](https://opentracing.io/) traces using [Jaeger](https://www.jaegertracing.io), run with `-trace`.

> **Note:**
>
> Since the Jaeger package used by the shim needs to communicate trace
> information to the Jaeger agent, it is necessary to ensure the shim runs in
> the same network namespace as the Jaeger agent.
