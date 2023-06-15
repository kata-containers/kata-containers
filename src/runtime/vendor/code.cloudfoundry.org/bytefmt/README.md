bytefmt
=======

**Note**: This repository should be imported as `code.cloudfoundry.org/bytefmt`.

Human-readable byte formatter.

Example:

```go
bytefmt.ByteSize(100.5*bytefmt.MEGABYTE) // returns "100.5M"
bytefmt.ByteSize(uint64(1024)) // returns "1K"
```

For documentation, please see http://godoc.org/code.cloudfoundry.org/bytefmt

## Reporting issues and requesting features

Please report all issues and feature requests in [cloudfoundry/diego-release](https://github.com/cloudfoundry/diego-release/issues).
