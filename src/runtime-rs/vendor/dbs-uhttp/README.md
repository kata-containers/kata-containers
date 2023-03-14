# dbs-uhttp

This is a minimal implementation of the
[HTTP/1.0](https://tools.ietf.org/html/rfc1945) and
[HTTP/1.1](https://www.ietf.org/rfc/rfc2616.txt) protocols. This HTTP
implementation is stateless thus it does not support chunking or compression.

## Acknowledgement

The `dbs-uhttp` repository is forked from Fireckracker's [micro-http](https://github.com/firecracker-microvm/micro-http)
, in order to:
- support more http request types.
- support other OSs like macOS by replacing vmm-sys-util::Poll with platfrom independent mio crate.

## Contributing

To contribute to dbs-uhttp, checkout the
[contribution guidelines](CONTRIBUTING.md).

## Releases

New dbs-uhttp versions are released via the GitHub repository releases page.
A history of changes is recorded in our [changelog](CHANGELOG.md).

## Policy for Security Disclosures

If you suspect you have uncovered a vulnerability, contact us privately, as outlined in our
[security policy document](); we will immediately prioritize your disclosure.