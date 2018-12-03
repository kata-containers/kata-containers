[![Build Status](https://travis-ci.org/kata-containers/shim.svg?branch=master)](https://travis-ci.org/kata-containers/shim)
[![codecov](https://codecov.io/gh/kata-containers/shim/branch/master/graph/badge.svg)](https://codecov.io/gh/kata-containers/shim)

# Shim

This project implements a shim called `kata-shim` for the [Kata
Containers](https://katacontainers.io/) project.

The shim runs in the host environment, handling standard I/O and signals on
behalf of the container process which runs inside the virtual machine.
