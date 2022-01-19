# Virtual Machine Manager for Go

[![Go Report Card](https://goreportcard.com/badge/github.com/kata-containers/govmm)](https://goreportcard.com/report/github.com/kata-containers/govmm)
[![Build Status](https://travis-ci.org/kata-containers/govmm.svg?branch=master)](https://travis-ci.org/kata-containers/govmm)
[![GoDoc](https://godoc.org/github.com/kata-containers/govmm/qemu?status.svg)](https://godoc.org/github.com/kata-containers/govmm/qemu)
[![Coverage Status](https://coveralls.io/repos/github/kata-containers/govmm/badge.svg?branch=master)](https://coveralls.io/github/kata-containers/govmm?branch=master)

Virtual Machine Manager for Go (govmm) is a suite of packages that
provide Go APIs for creating and managing virtual machines.  There's
currently support for only one hypervisor, qemu/kvm (version 5.0 and
later), support for which is provided by the
github.com/kata-containers/govmm/qemu package.

The qemu package provides APIs for launching qemu instances and for
managing those instances via QMP, once launched.  VM instances can
be stopped, have devices attached to them and monitored for events
via the qemu APIs.

The qemu package has no external dependencies apart from the Go
standard library and so is nice and easy to vendor inside other
projects.
