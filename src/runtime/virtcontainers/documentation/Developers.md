# Prerequisites

`virtcontainers` has a few prerequisites for development:

- CNI
- golang

# Building

To build `virtcontainers`, at the top level directory run:

```bash
# make
```

# Testing

Before testing `virtcontainers`, ensure you have met the [prerequisites](#prerequisites).

To test `virtcontainers`, at the top level run:

```
# make check
```

This will:

- run static code checks on the code base.
- run `go test` unit tests from the code base.

# Submitting changes

For details on the format and how to submit changes, refer to the
[Contributing](../../../../CONTRIBUTING.md) document.
