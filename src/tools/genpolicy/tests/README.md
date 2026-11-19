# `genpolicy` integration tests

The policy generation and integration tests execute the Cargo-built `genpolicy`
binary in a separate process. Unexpected exit outcomes are retried up to six
times, with a ten-second delay between attempts, to tolerate transient image
registry failures without changing production error handling. Exhausted retries
fail the test with the child's exit status, stdout, and stderr.

Policy tests supply pristine YAML through stdin on every attempt and decode
initdata only after a successful exit. Output decoding and policy allow/deny
assertions are not retried. Negative generation tests require both a failed exit
and the expected error message.
